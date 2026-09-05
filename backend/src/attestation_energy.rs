//! Attestation Energy — thin domain adapter over the `negentropy` physics
//! engine for ranking ZK attestations.
//!
//! The core thermodynamic formula (route energy, committor, negentropy
//! extraction) lives in the [`negentropy`] crate. This module maps
//! attestation domain quantities onto that engine:
//!
//! - **confidence** ← `attestation_type.base_depth() × issuer_trust`
//! - **depth_ratio** ← confidence / log₁₀(threshold)
//! - **timing_factor** ← exp(-age / half_life)
//! - **latency_decay** ← 1 / (1 + total_latency × decay_rate)
//! - **cost_penalty** ← (HCS + HTS cost in HBAR) × HBAR price, normalized
//!
//! See <https://github.com/orkid-labs/negentropy> for the physics.

use serde::{Deserialize, Serialize};

use negentropy::{Committor, Negentropy, RouteEnergy};

/// Attestation type — determines the confidence depth baseline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AttestationType {
    Age = 0,
    Income = 1,
    Credential = 2,
}

impl AttestationType {
    pub fn from_u64(v: u64) -> Option<Self> {
        match v {
            0 => Some(Self::Age),
            1 => Some(Self::Income),
            2 => Some(Self::Credential),
            _ => None,
        }
    }

    /// Base confidence depth — analogous to pool TVL in the FMD engine.
    /// Government-issued credentials have higher depth than self-attested data.
    pub fn base_depth(&self) -> f64 {
        match self {
            Self::Age => 100.0,        // ID-backed age: high confidence
            Self::Income => 50.0,      // Income: medium (bank statement)
            Self::Credential => 80.0,  // Credential: high (issued by authority)
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Age => "age",
            Self::Income => "income",
            Self::Credential => "credential",
        }
    }
}

/// Attestation energy evaluation result.
///
/// Produced by [`AttestationPotential::energy`]. The fields mirror the core
/// `negentropy::RouteEnergyResult` plus domain-specific extras (committor
/// and negentropy bits).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttestationEnergyResult {
    /// Total energy score (higher = better quality attestation)
    pub energy: f64,
    /// Confidence depth ratio (credential strength / threshold strictness)
    pub depth_ratio: f64,
    /// Timing factor (recency decay, 0..1)
    pub timing_factor: f64,
    /// Latency decay (proof gen + verify speed, 0..1)
    pub latency_decay: f64,
    /// Cost penalty (HBAR cost of HCS + HTS, 0..1)
    pub cost_penalty: f64,
    /// Committor probability (likelihood attestation is valid & uncontested)
    pub committor: f64,
    /// Negentropy extracted (information created by the proof, in bits)
    pub negentropy_bits: f64,
}

/// Configuration for attestation energy evaluation (chain-specific costs).
#[derive(Debug, Clone)]
pub struct AttestationPotential {
    /// HBAR cost per HCS message submission (in HBAR)
    pub hcs_cost_hbar: f64,
    /// HBAR cost per HTS NFT mint (in HBAR)
    pub hts_mint_cost_hbar: f64,
    /// HBAR price in USD (for cost normalization)
    pub hbar_price_usd: f64,
    /// Proof generation latency in milliseconds
    pub proof_latency_ms: u64,
    /// Verification latency in milliseconds
    pub verify_latency_ms: u64,
    /// Attestation age in seconds (time since proof was generated)
    pub attestation_age_secs: f64,
    /// Circuit constraint count (more constraints = more negentropy)
    pub constraint_count: u64,
}

impl Default for AttestationPotential {
    fn default() -> Self {
        Self {
            hcs_cost_hbar: 0.0001,     // HCS message: ~$0.0001 at $0.05/HBAR
            hts_mint_cost_hbar: 0.001, // HTS NFT mint: ~$0.05
            hbar_price_usd: 0.05,
            proof_latency_ms: 800,
            verify_latency_ms: 30,
            attestation_age_secs: 0.0,
            constraint_count: 27,
        }
    }
}

/// Half-life for attestation recency decay (1 hour, in seconds).
const HALF_LIFE_SECS: f64 = 3600.0;

impl AttestationPotential {
    /// Evaluate attestation energy via the `negentropy` physics engine.
    ///
    /// Delegates:
    /// - energy → `negentropy::RouteEnergy::new`
    /// - committor → `negentropy::Committor::score`
    /// - negentropy_bits → `negentropy::Negentropy::from_constraints`
    pub fn energy(&self, attestation_type: AttestationType, threshold: u64, issuer_trust: f64) -> AttestationEnergyResult {
        // Domain mapping: base depth × issuer trust → confidence
        let confidence = attestation_type.base_depth() * issuer_trust.clamp(0.0, 1.0);

        // Depth ratio: confidence relative to threshold strictness
        let threshold_f = threshold.max(1) as f64;
        let depth_ratio = confidence / threshold_f.log10().max(1.0);

        // Timing factor: exponential decay based on attestation age
        let timing_factor = (-self.attestation_age_secs / HALF_LIFE_SECS).exp();

        // Latency decay: total proof + verify latency
        let total_latency = self.proof_latency_ms + self.verify_latency_ms;
        let latency_decay = 1.0 / (1.0 + total_latency as f64 * 0.0001);

        // Cost penalty: HCS + HTS cost in USD, normalized
        let total_cost_usd = (self.hcs_cost_hbar + self.hts_mint_cost_hbar) * self.hbar_price_usd;
        let cost_penalty = (total_cost_usd * 0.01).min(0.5);

        // Core energy from negentropy
        let energy = RouteEnergy::new(
            confidence,
            depth_ratio,
            timing_factor,
            latency_decay,
            cost_penalty,
        )
        .energy;

        // Committor from negentropy (TPS rare-event prediction)
        let committor = Committor::score(depth_ratio, timing_factor, cost_penalty);

        // Negentropy extracted: N = constraint_count × log₂(threshold)
        let negentropy_bits =
            Negentropy::from_constraints(self.constraint_count, threshold).bits();

        AttestationEnergyResult {
            energy,
            depth_ratio,
            timing_factor,
            latency_decay,
            cost_penalty,
            committor,
            negentropy_bits,
        }
    }
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;

    #[test]
    fn test_age_attestation_energy() {
        let pot = AttestationPotential::default();
        let result = pot.energy(AttestationType::Age, 18, 0.95);

        assert!(result.energy > 0.0, "energy should be positive");
        assert!(result.depth_ratio > 0.0);
        assert!(result.timing_factor > 0.99, "fresh attestation should have high timing");
        assert!(result.latency_decay > 0.0);
        assert!(result.committor > 0.0 && result.committor <= 1.0);
        assert!(result.negentropy_bits > 0.0);
    }

    #[test]
    fn test_stale_attestation_decays() {
        let mut pot = AttestationPotential::default();
        pot.attestation_age_secs = 7200.0; // 2 hours = 2 half-lives

        let fresh = AttestationPotential::default().energy(AttestationType::Age, 18, 0.9);
        let stale = pot.energy(AttestationType::Age, 18, 0.9);

        assert!(
            stale.energy < fresh.energy,
            "stale attestation should have lower energy"
        );
        assert!(
            stale.timing_factor < fresh.timing_factor * 0.5,
            "2 half-lives should reduce timing by >50%"
        );
    }

    #[test]
    fn test_higher_threshold_more_negentropy() {
        let pot = AttestationPotential::default();

        let low_threshold = pot.energy(AttestationType::Income, 30_000, 0.8);
        let high_threshold = pot.energy(AttestationType::Income, 100_000, 0.8);

        assert!(
            high_threshold.negentropy_bits > low_threshold.negentropy_bits,
            "higher threshold extracts more negentropy"
        );
    }

    #[test]
    fn test_low_trust_reduces_energy() {
        let pot = AttestationPotential::default();

        let high_trust = pot.energy(AttestationType::Credential, 999, 0.95);
        let low_trust = pot.energy(AttestationType::Credential, 999, 0.3);

        assert!(
            low_trust.energy < high_trust.energy,
            "lower issuer trust should reduce energy"
        );
    }

    #[test]
    fn test_negentropy_formula() {
        // 27 constraints, threshold 18: N = 27 * log2(18) ≈ 112.6 bits
        let pot = AttestationPotential::default();
        let result = pot.energy(AttestationType::Age, 18, 0.9);
        let expected = 27.0 * (18.0f64).log2();
        assert!(
            (result.negentropy_bits - expected).abs() < 0.01,
            "negentropy should be 27 * log2(18) ≈ {:.1}, got {:.1}",
            expected,
            result.negentropy_bits
        );
    }
}
