use std::any::Any;

use bls12_381::hash_to_curve::{ExpandMsgXmd, HashToCurve};
use bls12_381::{
    multi_miller_loop, G1Affine, G1Projective, G2Affine, G2Prepared, G2Projective, Gt, Scalar,
};
use eyre::{eyre, Result};
use serde::{Deserialize, Serialize};
use ssz_derive::{Decode, Encode};
use tree_hash_derive::TreeHash;
use typenum;

use crate::bytes::*;
use crate::crypto::{Point, PublicKey, Signature};

#[derive(Debug, Clone, Default, Serialize, Deserialize, Encode, Decode, TreeHash)]
#[ssz(struct_behaviour = "transparent")]
#[serde(transparent)]
pub struct BlsPublicKey {
    data: ByteVector<typenum::U48>,
}

impl PublicKey for BlsPublicKey {
    fn point(&self) -> Result<Box<dyn Point>, eyre::Error> {
        let bytes: [u8; 48] = self.data.data[..].try_into()?;
        let ct_option = G1Affine::from_compressed(&bytes);

        if ct_option.is_some().into() {
            Ok(Box::new(ct_option.unwrap()))
        } else {
            Err(eyre!("invalid point"))
        }
    }

    fn as_any(&self) -> &dyn Any { self }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, Encode, Decode, TreeHash)]
#[ssz(struct_behaviour = "transparent")]
#[serde(transparent)]
pub struct BlsSignature {
    data: ByteVector<typenum::U96>,
}

impl Signature for BlsSignature {
    /// PublicKeys must all be verified via Proof of Possession before running
    /// this function. https://tools.ietf.org/html/draft-irtf-cfrg-bls-signature-02#section-3.3.4
    fn verify(&self, msg: &[u8], pks: &[Vec<Box<dyn PublicKey>>]) -> bool {
        let sig_point = match self.point() {
            Ok(point) => point,
            Err(_) => return false,
        };
        // Subgroup check for signature
        if !bls_subgroup_check_g2(
            sig_point
                .as_ref()
                .as_any()
                .downcast_ref::<G2Affine>()
                .unwrap(),
        ) {
            return false;
        }

        // Extract BlsPublicKey from Vec<Box<dyn PublicKey>>
        let bls_pks: Vec<BlsPublicKey> = pks
            .iter()
            .flat_map(|pk_vec| pk_vec.iter())
            .filter_map(|pk| pk.as_any().downcast_ref::<BlsPublicKey>().cloned())
            .collect();

        // Aggregate PublicKeys
        let aggregate_public_key = match aggregate_bls_keys(&bls_pks) {
            Ok(agg) => agg,
            Err(_) => return false,
        };

        // Ensure AggregatePublicKey is not infinity
        if aggregate_public_key.is_identity().into() {
            return false;
        }

        // Points must be affine for pairing
        let msg_hash = G2Affine::from(bls_hash_to_curve(msg));
        // Faster ate2 evaluation checks e(S, -G1) * e(H, PK) == 1
        bls_evaluate_sig_eq(
            sig_point
                .as_ref()
                .as_any()
                .downcast_ref::<G2Affine>()
                .unwrap(),
            &msg_hash,
            &aggregate_public_key,
        )
    }

    fn point(&self) -> Result<Box<dyn Point>, eyre::Error> {
        let bytes: [u8; 96] = self.data.data[..].try_into()?;
        let ct_option = G2Affine::from_compressed(&bytes);

        if ct_option.is_some().into() {
            Ok(Box::new(ct_option.unwrap()))
        } else {
            Err(eyre!("invalid point"))
        }
    }
}

/// Aggregates multiple keys into one aggregate key
fn aggregate_bls_keys(pks: &[BlsPublicKey]) -> Result<G1Affine> {
    if pks.is_empty() {
        return Err(eyre!("no keys to aggregate"));
    }

    let agg_key = pks.iter().try_fold(G1Projective::identity(), |acc, key| {
        let point = key.point()?;
        Ok::<_, eyre::Report>(
            acc + G1Projective::from(point.as_any().downcast_ref::<G1Affine>().unwrap()),
        )
    })?;

    Ok(G1Affine::from(agg_key))
}

/// Verifies a G2 point is in subgroup `r`.
fn bls_subgroup_check_g2(point: &G2Affine) -> bool {
    const CURVE_ORDER: &str = "73EDA753299D7D483339D80809A1D80553BDA402FFFE5BFEFFFFFFFF00000001";
    let r = bls_hex_to_scalar(CURVE_ORDER).unwrap();
    (point * r).is_identity().into()
}

fn bls_evaluate_sig_eq(
    signature_point: &G2Affine,
    msg_curve_hash: &G2Affine,
    aggregate_key: &G1Affine,
) -> bool {
    let generator_g1_negative = G1Affine::from(-G1Projective::generator());
    // Prepare G2 points for efficient pairing
    let signature_prepared = G2Prepared::from(*signature_point);
    let msg_hash_prepared = G2Prepared::from(*msg_curve_hash);

    // Compute e(S, -G1) * e(H, PK) with miller loop
    let pairing = multi_miller_loop(&[
        (&generator_g1_negative, &signature_prepared),
        (aggregate_key, &msg_hash_prepared),
    ]);
    pairing.final_exponentiation() == Gt::identity()
}

fn bls_hash_to_curve(msg: &[u8]) -> G2Projective {
    const DST: &[u8] = b"BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_POP_";
    <G2Projective as HashToCurve<ExpandMsgXmd<sha2::Sha256>>>::hash_to_curve(msg, DST)
}

fn bls_hex_to_scalar(hex: &str) -> Option<Scalar> {
    if hex.len() != 64 {
        return None;
    }

    let raw: Result<Vec<u64>, _> = hex
        .as_bytes()
        .chunks(16)
        .map(|chunk| {
            core::str::from_utf8(chunk)
                .map_err(|_| eyre!("invalid UTF-8")) // Convert Result to Result with custom error
                .and_then(|hex_chunk| {
                    u64::from_str_radix(hex_chunk, 16).map_err(|_| eyre!("invalid hex"))
                })
                .map(u64::to_le)
        })
        .collect::<Result<Vec<u64>, _>>();

    raw.ok().and_then(|vec| {
        if vec.len() == 4 {
            let mut raw_array = [0u64; 4];
            for (i, &value) in vec.iter().enumerate() {
                raw_array[3 - i] = value;
            }
            Some(Scalar::from_raw(raw_array))
        } else {
            None
        }
    })
}

impl Point for G2Affine {
    fn as_any(&self) -> &dyn Any { self }
}
impl Point for G1Affine {
    fn as_any(&self) -> &dyn Any { self }
}
