use scrypto::prelude::*;

#[derive(ScryptoSbor, NonFungibleData)]
pub struct UserReceipt {
  #[mutable]
  // counts the XRD in protocol (the debt will be mandatorily pegged)
  // gets updated remotely (can it? would be nice if it could)
  // should work: TODO: store the UUID id in the CDP contract
  protocol_lp: Decimal
  #[mutable]
  // REAL (or REAL/XRD LP), protocol LP, eUSD/XRD LP
  staked_token_amount: (Decimal, Decimal, Decimal), 
  #[mutable]
  // amount "claimed" per rewards vault
  rewards_claimed: Vec<Decimal>, 
  top_voted_index: u64 // can't vote for lower than this 
}

// no, fuck you, I am not going to import a crate
// just for these two little silly enums
#[derive(ScryptoSbor)]
pub enum Vote {
  For(Decimal),
  Against(Decimal),
  Abstain(Decimal)
}

#[derive(ScryptoSbor)]
pub enum Proposal {
  TextOnly(String),
  ActionSequence(Vec<Action>),
  ChangeGovGuardian(Addr, Addr) 
}

external_component!(
  Gov {
    fn vote(&mut self, vote: Vote, proposal: Proposal, proposal_idx: u64);
  }
)

type RewardsTally = (Vault, Decimal);

#[blueprint]
mod staking {
  // contract that handles staking of the governance token
  // distribution of direct fee sharing and vote power tallying
  struct Staking {
    gov_token: ResourceAddres,
    lp_token: ResourceAddres,

    voting_badge: Vault,
    receipt_badge: Vault, 
    receipt_resource: ResourceAddress,
    // total currently staked
    // ($REAL (or REAL/XRD LP), eUSD/XRD LP) 
    // note: latter 2 need to be enabled TODO
    total_staked: (Decimal, Decimal), 
    // REAL (protocol lp), XRD (fee share to REAL), 
    // eUSD (fee share to REAL), REAL (eUSD/XRD lp)
    rewards: (RewardsTally, RewardsTally, RewardsTally, RewardsTally),

    gov_address: ComponentAddress,

    // parameters
    
    // Some -- enabled, can't be changed
    // the lp token equated to Dex LP (dlp)
    // and the formula's scaling factor
    dlp: Option<(ResourceAddres, Decimal)>

  }

  impl Staking {
    pub fn instantiate_staking(
      token: ResourceAddress, 
      voting_badge: Bucket,
      receipt_badge: Bucket,
      receipt_resource: ResourceAddres) { // -> ComponentAddress {

      // b1 (REAL issuance for protocol LP i.e. CDPs) 
      // b2 (XRD fee share for REAL stakers)
      // b3 (eUSD fee share for REAL stakers)
      // b4 (REAL issuance for DEX LP on eUSD/XRD)
      // the core idea being that
      // PLP ~ Protocol LP, DLP ~ Dex LP

      // b1 = PLP * f(REAL, PLP, DLP)
      // b2, b3 = REAL * g(REAL, PLP, DLP)
      // b4 = DLP * h(REAL, PLP, DLP)

      // direct PLP staking is a problem
      // as it makes the calculations waaay off
      // would be better IMO if the stake was removed
      // and it was just (REAL/XRD) and (eUSD/XRD) stakes
      // or LXRD 
      // effective portfolio 50 XRD 25 REAL 25 eUSD
      // would be better if REAL/eUSD was a thing
      // or would it???
      // but definitely, 
      // would be better with an 80/20 REAL/XRD lp
      // then becoming 40 REAL 35 XRD 25 eUSD

      // leaving above for now, but it's outdated
      // b1 (XRD fee share for REAL stakers)
      // b2 (eUSD fee share for REAL stakers)
      // b3 (REAL issuance for DEX LP on eUSD/XRD)

      // weird idea: what if the formula operated like a DEX
      // i.e. have a pair of (REAL) / (eUSD/XRD)
      // that accordingly updates the correct distributions 
      // incentivising a 

    }

    // anyone can deposit cause free money
    pub fn deposit_rewards(&mut self, index: u64, input: Bucket) {
      // update rewards[index]
      // check resource type
    }

    // AuthZone: something? 
    // adds another rewards type
    pub fn add_vault() {
      // should probably keep it to update only thing
      // as it fucks with the curves
    }

    // AuthZone: superbadge?
    // enables staking of the LP tokens
    pub fn enable_lp_staking(&mut self, ideal_ratio: Decimal) {
      // ideal ratio is both a scaling factor and used to determine the 
      // boosting amount on an LP per real basis (need enough for full boost)
      // probably should do like 0.1% TVL needs X * 0.1% supply for full boost
      // ^ keeping here for legacy sake for now

      // TODO do this for both PLP and DLP
    }

    // when adding stake, it doesn't 'vote up' the vote
    // i.e. any votes for pending proposals get lost
    pub fn stake() {
      // Remember to check/update unclaimed to init token_amount 
      // in case new rewards type was added 
    }

    pub fn unstake() {

    }

    // could remove the proposal arg from this + upstream if it gets too big
    // or annoying, it's just a double check against accidental votes when prior finalizes
    pub fn vote(&self, receipt: Proof, vote: Vote, proposal: Proposal, proposal_idx: u64) {
      let nft: NonFungible<StakingReceipt> = receipt
        .validate_proof(self.receipt_resource)
        .expect("wrong resource")
        .non_fungible();
      let mut nft_data = nft.data();

      assert!( nft_data.top_voted_index < proposal_idx, "double vote" );
      // I know
      match vote {
        For(x) => assert!(x == nft_data.token_amount, "wrong vote size"),
        Against(x) => assert!(x == nft_data.token_amount, "wrong vote size"),
        Abstain(x) => assert!(x == nft_data.token_amount, "wrong vote size"),
      };
      
      self.voting_badge.authorize(|| 
        Gov::at(self.gov_address).vote(vote, proposal, proposal_idx)
      );
      self.receipt_badge.authorize(|| 
        borrow_resource_manager!(self.receipt_resource).update_non_fungible_data(
          &nft.local_id(),
          "top_voted_index",
          proposal_idx
        )
      );

    }

    // claims rewards
    // for now the only reward type is XRD
    pub fn claim(&self, receipt: Proof, index: u64) -> Bucket {
      // TODO update to 4 vault system
      // (after formulas figured out)

      // TODO add a deposit-of-last-resort 
      // that 
      let nft: NonFungible<StakingReceipt> = receipt
        .validate_proof(self.receipt_resource)
        .expect("wrong resource")
        .non_fungible();
      let mut nft_data = nft.data();

      let available = 
        self.rewards[index] * nft_data.token_amount - nft_data.claimed[index];
      
      assert!(available > dec!(0), "nonpositive claim");

      self.receipt_badge.authorize(|| 
        borrow_resource_manager!(self.receipt_resource).update_non_fungible_data(
          &nft.local_id(),
          "rewards_claimed",
          self.rewards[index] * nft_data.token_amount
        )
      );
      self.rewards[index].take(available) 
    }
  }
}