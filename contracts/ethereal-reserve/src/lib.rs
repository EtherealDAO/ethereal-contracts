use scrypto::prelude::*;

#[blueprint]
mod reserve {
  // the reserve contract governs the token issuance 
  // by decoupling it from the usd script it allows
  // granular disbursal control (and to many targets)
  struct Reserve {
    token: Vault,
    token_issued: Decimal,
    token_total: Decimal,
    issue_badge: ResourceAddress,
    reserve_guardian_badge: ResourceAddress,
    upgrade_badge: ResourceAddres // Q: use dao_superbadge for this? or power zero?
  }

  impl Reserve {
    // the reserve has one token it governs
    // returns the badge 
    // doubles down as a from_nothing and from_something
    pub fn instantiate_reserve(
      tkn: Bucket, 
      tkn_total: Decimal, // tkn_total - tkn.amount() = issued
      issue_badge: ResourceAddress, // issue token
      guardian_badge: ResourceAddress, // set limit on token issue
      upgrade_badge: ResourceAddress // rip the soul out
      ) 
      -> ComponentAddress
      {

    }

    // AuthRule: issue_badge 
    // issues amount of the token
    // may be later constrained in the amount 
    // so that it follows a distribution curve
    pub fn issue(&mut self, amount: Decimal) -> Bucket {
      // idea: only allow issuance to/by an `ethereal-distributor` type script
      // that knows exactly how much and when to distribute
      // then per project/source of tokens they'd have their own distributor
      // or have it all in one idk that could also work
    }

    // AuthRule: reserve_guardian_badge
    // sets the new issue limit 
    // (up to how much of tkn can be issues)
    pub fn set_limit(&mut self, new_limit: Decimal) {

    }

    // AuthRule: upgrade_badge
    // rips all the tokens out for upgrade purposes
    // technically equivalent to guardian and issue conspiration
    pub fn to_nothing(&mut self) -> Bucket {

    } 

    // returns (Available supply, Current Issue limit, Total supply)
    pub fn status(&self) -> (Decimal, Decimal, Decimal) {

    }

    // returns the rest of the state 
    // separated for nominal efficency
    pub fn badges(&self) -> (ResourceAddres, ResourceAddres, ResourceAddres) {

    }
    
  }
}