<p align="center">
  <a href="https://www.orkidlabs.com"><img src="assets/logo.png" alt="Orkid Labs" width="220" /></a>
</p>

# zk-attest

**By [Orkid Labs](https://www.orkidlabs.com)** — privacy-first crypto engineering

**Zero-knowledge attestation platform on Hedera — prove without revealing.**

zk-attest lets users prove attributes about themselves (age, income, credential possession) using Groth16 zero-knowledge proofs, without revealing the underlying private data. Each attestation is logged to Hedera Consensus Service (HCS) for an immutable audit trail and minted as an HTS NFT — a transferable, verifiable credential.

> **Status:** The issuer signature scheme uses a **Poseidon-based Schnorr-like signature** (`pk = Poseidon(sk)`, `sig = sk + Poseidon(pk, m, r)`, verified in-circuit via `Poseidon(sig - h) === pk`). Forging requires a preimage attack on Poseidon over BN254. The Hedera HCS/HTS integration is currently **simulated** — `hiero-sdk` is wired in but no real transactions are submitted until credentials are configured.

Attestations are scored using an **attestation energy model** adapted from the [orkid FMD physics engine](https://github.com/orkid-labs/orkid) — the same thermodynamic framework that scores arbitrage routes in production MEV extraction.

> **Note:** The orkid repository is private. Access can be provided to
> Thrive Protocol reviewers and other appropriate cases on request —
> contact [Orkid Labs](https://www.orkidlabs.com). The theoretical
> foundation is published as a preprint:
> ["Negative EV per Unit Time as Blockchain Inefficiency"](https://www.researchgate.net/publication/399474539_Negative_EV_per_Unit_Time_as_Blockchain_Inefficiency)
> — [Jacob Cavazos, ResearchGate](https://www.researchgate.net/profile/Jacob-Cavazos).

## Why this matters

Every age-gated website, income-verified service, and credential-checked platform today collects raw PII — birthdates, salary, ID numbers. This creates:

- **PII liability**: GDPR exposure, breach risk, compliance overhead
- **Data honeypots**: Centralized stores of sensitive data attract attackers
- **Privacy erosion**: Users surrender personal data for trivial verifications

zk-attest eliminates this. Users prove `age >= 18` or `income >= 50000` with a ZK proof. The verifier learns **only** the boolean result — never the underlying value.

## Architecture

| Layer | Technology |
|-------|-----------|
| Circuit | circom 2.2.3 (Groth16, BN128, 27 non-linear constraints) |
| Backend | Rust (axum, tokio, hiero-sdk 0.45) |
| Hedera | HCS (audit log) + HTS (NFT minting) + Mirror Nodes (verification) |
| Frontend | Vanilla HTML/JS (zero dependencies) |
| Energy model | FMD route energy formula (adapted from orkid fmd-physics) |

## The circuit

`circuit/attest.circom` supports three attestation types in a single circuit:

| Type | Private input | Public output | Privacy guarantee |
|------|--------------|---------------|-------------------|
| 0 (Age) | birth_year | `age >= threshold` | Birth year never revealed |
| 1 (Income) | annual_income | `income >= threshold` | Income never revealed |
| 2 (Credential) | credential_id | `credential_id === expected` | Credential ID never revealed |

**Public signals**: `[attestation_type, threshold, issuer_pubkey_hash, current_year]`
**Private signals**: `[private_value, issuer_signature, signature_randomness]`

The circuit constrains:
1. **Signature verification**: `sig = private_value + pubkey_hash * randomness`
2. **Attestation logic**: type-dependent range check (age: `current_year - birth_year >= threshold`, income: `income >= threshold`, credential: exact match)
3. **Type selection**: exactly one attestation type is active (IsEqual constraints)

## Hedera integration

> **Current state:** The Hedera integration is **simulated** until real credentials are configured. The `hiero-sdk` dependency is wired in and the code structure is ready, but `submit_attestation` and `submit_verification` return mock sequence numbers when no credentials are present. To enable real transactions, set `HEDERA_OPERATOR_ID` and `HEDERA_OPERATOR_KEY` environment variables.

When fully wired, each attestation generates **3+ Hedera transactions**:

| Transaction | Service | Purpose |
|------------|---------|---------|
| 1 | HCS | Proof + public signals logged to consensus topic |
| 2 | HTS | NFT minted (attestation tokenized as transferable credential) |
| 3 | HCS | Energy score + verification result logged |
| 4 | HCS | Verification confirmation (on verify) |

### Hedera services used

- **HTS (Hedera Token Service)**: Mint NFTs representing attestations. Each NFT's metadata contains the proof ID, attestation type, and energy score. NFTs are transferable — users can present them to third-party verifiers.
- **HCS (Hedera Consensus Service)**: Immutable, timestamped, ordered log of all attestation proofs and verification events. Creates a tamper-proof audit trail accessible via Mirror Nodes.
- **Mirror Nodes**: Third-party verifiers query HCS topic messages to independently verify attestation history without trusting the issuer.

### Configuration

```bash
# Testnet (simulated mode without credentials)
export HEDERA_NETWORK=testnet

# Mainnet (real transactions)
export HEDERA_NETWORK=mainnet
export HEDERA_OPERATOR_ID=0.0.xxxx
export HEDERA_OPERATOR_KEY=302e0201...
export HEDERA_TOPIC_ID=0.0.xxxx   # optional, auto-created
export HEDERA_TOKEN_ID=0.0.xxxx   # optional, auto-created
```

## Attestation energy model (FMD physics)

The attestation energy model is adapted from the **route energy formula** in the orkid FMD (Financial Molecular Dynamics) physics engine — a production MEV detection system that treats market inefficiency as thermodynamic negentropy.

### The thermodynamic framing

In the orkid framework:
- **Markets** are thermodynamic systems with entropy (disorder) and negentropy (order)
- **Arbitrage opportunities** are low-entropy pockets — mispricings that searchers extract as profit
- **Route energy** scores arbitrage paths by how much negentropy they extract

In zk-attest:
- **Private data** is a high-entropy state (chaotic, unverifiable)
- **A ZK proof** is a negentropy extraction — converting private chaos into structured, verifiable order
- **Attestation energy** scores proofs by how much negentropy they extract

### The formula

**FMD route energy** (orkid `fmd-physics/src/route_energy.rs`):
```
energy = net_bps * sqrt(depth_ratio * timing_factor) * latency_decay * (1 - gas_penalty)
```

**Attestation energy** (adapted):
```
energy = confidence * sqrt(depth_ratio * timing_factor) * latency_decay * (1 - cost_penalty)
```

Where:
- **confidence** = `base_depth(attestation_type) * issuer_trust` — analogous to pool TVL
- **depth_ratio** = `confidence / log10(threshold)` — higher threshold = harder to prove = more negentropy
- **timing_factor** = `exp(-age / half_life)` — recency decay (1-hour half-life)
- **latency_decay** = `1 / (1 + total_latency_ms * 0.0001)` — proof generation speed
- **cost_penalty** = `(hcs_cost + hts_cost) * hbar_price * normalization` — Hedera transaction cost

### Negentropy extraction

Each ZK proof extracts negentropy (information) from private data:

```
N = constraint_count * log2(threshold)
```

For a 27-constraint circuit proving age >= 18: `N = 27 * log2(18) ≈ 112.6 bits`

This is the Shannon entropy reduction — the amount of uncertainty eliminated by the proof.

### Committor function

Adapted from the TPS (Transition Path Sampling) committor in the FMD engine, which predicts the probability of reaching a profitable state:

```
committor = (depth_ratio / (1 + depth_ratio)) * timing_factor * (1 - cost_penalty * 0.5)
```

This estimates the probability that an attestation is valid and uncontested — a "rare event" prediction for attestation quality.

## Thrive zkVerify Web3 Program (#45) — Grant Plan

### Ecosystem value proposition

zk-attest drives **proof verification volume** to zkVerify. Each attestation generates a ZK proof that is submitted to zkVerify for verification. The attestation is then logged to Hedera HCS and minted as an HTS NFT — the proof verification happens via zkVerify, the settlement happens on Hedera.

| Scenario | Attestations/month | Proofs to zkVerify/month |
|----------|-------------------|------------------------|
| Age-gated e-commerce | 8,000 | 8,000 |
| Income verification (fintech KYC) | 5,000 | 5,000 |
| Credential checks (gig platforms) | 3,000 | 3,000 |
| **Total (conservative)** | **16,000** | **16,000** |

**25,000+ ZK Proofs** (Milestone 2: Initial Traction target) is achievable with ~25,000 attestations/month — well within a multi-deployment scenario.

### Milestone roadmap

Progressive achievement over 150 days, following Thrive's zkVerify Web3 Program milestone structure.

**Application Requirements (10% unlocked at approval)**:
- ✅ Detailed technical plan showing how zero-knowledge proofs will be integrated and verified using zkVerify
- ✅ Zero-knowledge focused user experience design
- ✅ Token utility and ecosystem value proposition
- ✅ Business plan demonstrating revenue model and sustainability beyond grant period

**Milestone 1: Live Deployment (10% unlocked) — 45 days post approval**:
- Production deployment with fully functional zkVerify integration and proof verification
- Beta testing with proof verification validation
- Published documentation covering zkVerify integration and proof verification processes

**Milestone 2: Initial Traction (30% unlocked) — 90 days post approval**:
- Early traction metrics, choose one of the following:
  - Transaction Volume: 25,000+ ZK Proofs sent to zkVerify
  - Unique Users: 250+ unique addresses interacting with zkVerify integration

**Milestone 3: Scale (50% unlocked) — 150 days post approval**:
- Choose one of the following:
  - Transaction Volume: 250,000+ ZK Proofs sent to zkVerify
  - Unique Users: 2,500+ unique addresses interacting with zkVerify integration

## API

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/health` | GET | Health check + Hedera connection status (unauthenticated) |
| `/api/issue` | POST | Issue signed credential (simulated authority) |
| `/api/attest` | POST | Generate ZK proof + submit to HCS/HTS |
| `/api/verify` | POST | Verify a proof + log to HCS |
| `/api/stats` | GET | Metrics for milestone tracking |
| `/api/energy/:id` | GET | Attestation energy score (FMD model) |

All endpoints except `/api/health` require `x-api-key: $ORKID_API_KEY` in production.

### Production hardening

The backend is locked down for production deployment with two middleware layers:

1. **API key authentication** (`src/auth.rs`) — every sensitive endpoint requires the `x-api-key` header to match `ORKID_API_KEY`. `/api/health` remains exempt. Dev mode: if `ORKID_API_KEY` is unset, all requests are allowed.
2. **CORS restriction** (`src/auth.rs`) — cross-origin access is limited to the comma-separated list in `ORKID_CORS_ORIGINS`, defaulting to `http://localhost:3000`. Wildcard CORS is removed.

```bash
export ORKID_API_KEY="your-256-bit-secret"
export ORKID_CORS_ORIGINS="https://app.orkidlabs.xyz,http://localhost:3000"
```

## Quick start

```bash
# 1. Install circom + snarkjs
# Linux:   circom-linux-amd64
# macOS:   circom-darwin-amd64
# Windows: circom-windows-amd64.exe
curl -Ls https://github.com/iden3/circom/releases/latest/download/circom-linux-amd64 -o /usr/local/bin/circom
npm install -g snarkjs

# 2. Compile circuit + trusted setup
cd circuit && npm install circomlib
circom attest.circom --r1cs --wasm --sym -o ../build
cd ../build
snarkjs powersoftau new bn128 8 pot0_0000.ptau
snarkjs powersoftau contribute pot0_0000.ptau pot0_0001.ptau --name="zk-attest" -e="entropy"
snarkjs powersoftau prepare phase2 pot0_0001.ptau pot0_final.ptau
snarkjs groth16 setup attest.r1cs pot0_final.ptau attest_0000.zkey
snarkjs zkey contribute attest_0000.zkey attest_final.zkey --name="zk-attest" -e="entropy"
snarkjs zkey export verificationkey attest_final.zkey verification_key.json

# 3. Run backend
cd .. && cargo run

# 4. Open frontend
open http://localhost:3000
```

## Build & test

```bash
cargo build
cargo test --bin zk-attest-backend
```

## Project structure

```
zk-attest/
├── circuit/
│   ├── attest.circom          # Multi-attestation ZK circuit
│   └── package.json           # circomlib dependency
├── build/                     # Compiled circuit artifacts (gitignored)
├── backend/
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs            # axum server entry
│       ├── routes.rs          # HTTP API
│       ├── auth.rs            # API key + CORS middleware
│       ├── types.rs           # API types
│       ├── state.rs           # Metrics tracking
│       ├── issuer.rs          # Credential issuance (simulated authority)
│       ├── prover.rs          # snarkjs proof generation + verification
│       ├── hedera.rs          # HCS + HTS integration
│       └── attestation_energy.rs  # FMD physics energy model
├── frontend/
│   └── index.html             # Zero-dependency web UI
├── Cargo.toml                 # Workspace
└── README.md
```

## Production upgrades

- **EdDSA on BabyJubJub**: Upgrade from Poseidon-Schnorr to full EdDSA for non-repudiation
- **Merkle-tree issuer registry**: Multi-issuer support with on-chain revocation
- **Real Hedera integration**: Wire `hiero-sdk` for live HCS/HTS transactions (code structure ready)
- **Issuer marketplace**: Multiple credential authorities with on-chain reputation
- **Revocation**: HCS-based revocation registry
- **Batch verification**: Verify multiple proofs in a single HCS message

## About

Built by [Orkid Labs](https://www.orkidlabs.com) — a privacy-first crypto
engineering lab building thermodynamic infrastructure for decentralized
systems. See our other work at [orkidlabs.com](https://www.orkidlabs.com).

## License

MIT
