use std::marker::PhantomData;
use std::sync::atomic::Ordering;

use anyhow::{ensure, Result};
use storage_proofs::porep::stacked::{self, LayerChallenges, StackedDrg};
use storage_proofs::post::election::{self, ElectionPoSt};
use storage_proofs::proof::ProofScheme;
use typenum::Unsigned;

use crate::constants::{
    DefaultPieceHasher, DefaultTreeHasher, DEGREE, DRG_DEGREE, EXP_DEGREE, LAYERS,
    POREP_MINIMUM_CHALLENGES,
};
use crate::types::{PaddedBytesAmount, PoStConfig};

const DRG_SEED: [u8; 28] = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
    26, 27,
]; // Arbitrary, need a theory for how to vary this over time.

type PostSetupParams = election::SetupParams;
pub type PostPublicParams = election::PublicParams;

pub fn public_params(
    sector_bytes: PaddedBytesAmount,
    partitions: usize,
) -> Result<stacked::PublicParams<DefaultTreeHasher, DEGREE>> {
    StackedDrg::<DefaultTreeHasher, DefaultPieceHasher, DEGREE>::setup(&setup_params(
        sector_bytes,
        partitions,
    )?)
}

pub fn post_public_params(post_config: PoStConfig) -> Result<PostPublicParams> {
    ElectionPoSt::<DefaultTreeHasher>::setup(&post_setup_params(post_config))
}

pub fn post_setup_params(post_config: PoStConfig) -> PostSetupParams {
    let size = PaddedBytesAmount::from(post_config);

    election::SetupParams {
        sector_size: size.into(),
        challenge_count: post_config.challenge_count,
        challenged_nodes: post_config.challenged_nodes,
    }
}

pub fn setup_params(
    sector_bytes: PaddedBytesAmount,
    partitions: usize,
) -> Result<stacked::SetupParams<crate::constants::DEGREE>> {
    let layer_challenges = select_challenges(
        partitions,
        *POREP_MINIMUM_CHALLENGES
            .read()
            .unwrap()
            .get(&u64::from(sector_bytes))
            .expect("unknown sector size") as usize,
        *LAYERS
            .read()
            .unwrap()
            .get(&u64::from(sector_bytes))
            .expect("unknown sector size"),
    )?;
    let sector_bytes = u64::from(sector_bytes);

    ensure!(
        sector_bytes % 32 == 0,
        "sector_bytes ({}) must be a multiple of 32",
        sector_bytes,
    );

    let nodes = (sector_bytes / 32) as usize;
    let degree = DRG_DEGREE.load(Ordering::Relaxed) as usize;
    let expansion_degree = EXP_DEGREE.load(Ordering::Relaxed) as usize;
    assert_eq!(DEGREE::to_usize(), degree + expansion_degree);

    Ok(stacked::SetupParams {
        nodes,
        degree,
        expansion_degree,
        seed: DRG_SEED,
        layer_challenges,
        _degree: PhantomData,
    })
}

fn select_challenges(
    partitions: usize,
    minimum_total_challenges: usize,
    layers: usize,
) -> Result<LayerChallenges> {
    let mut count = 1;
    let mut guess = LayerChallenges::new(layers, count);
    while partitions * guess.challenges_count_all() < minimum_total_challenges {
        count += 1;
        guess = LayerChallenges::new(layers, count);
    }
    Ok(guess)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partition_layer_challenges_test() {
        let f = |partitions| {
            select_challenges(partitions, 12, 11)
                .unwrap()
                .challenges_count_all()
        };
        // Update to ensure all supported PoRepProofPartitions options are represented here.
        assert_eq!(6, f(usize::from(crate::PoRepProofPartitions(2))));

        assert_eq!(12, f(1));
        assert_eq!(6, f(2));
        assert_eq!(3, f(4));
    }
}
