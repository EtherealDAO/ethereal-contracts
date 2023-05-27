use scrypto::prelude::*;

// Something 
// likely a separate contract
// want proposals to be a list of permissions + fun/method calls
// EDSL-like

#[blueprint]
mod gov {
  // contract that handles voting upon proposals
  struct Gov {
    proposals: Vec<Something>,
    // map of hex Component/PackageAddress to Vec of Badges they can use
    // i.e. contract injection prevention
    whitelist: HashMap<String, Vec<ResourceAddres>>,
    gov_guardian_badge: ResourceAddres,

    // gov parameters

    vote_duration: Decimal, // duration of votes before allowed to close
    vote_quorum: Decimal, // minimum % participation before considered quorate

  }

  impl Gov {

    pub fn instantiate_gov() -> ComponentAddress {

    }

    // adds proposal to internal list of vote-able proposals
    pub fn add_proposal(&mut self, proposal_ref: Something) {

    }

    // AuthRule: voting authority? 
    // would mean vooting thru the staking contract 
    // and it handles all the vote logic, just passing in the raw vote power here
    // should be a good decision
    pub fn vote(&mut self, vote_type: Vote, , proposal_ref: Something) {

    }

    // either executes or not, depending on result
    // and then removes it out of the internal list 
    pub fn finalize_proposal(&mut self, proposal_ref: Something) {

    }

    // AuthRule: gov_guardian_badge
    // changes the vote whitelist
    // note: removal won't retroactively disallow a proposal's execution
    // i.e. the check is made upon submission, not execution
    pub fn modify_whitelist(&mut self, addr: String, new: Vec<ResourceAddres>) {

    }

    // AuthRule: ?? NOT guardian, this needs a vote passing 
    // so could just make it non-public? 
    pub fn modify_parameters(&mut self) {

    }

    // AuthRule: ??? dao superbadge or Power 0?
    pub fn to_nothing(&mut self) {
      // NEEDS THE proposal list to be empty 
      // TODO ASSERT IT
      // or actually this wouldn't work when voting for self-update so 
      // likely best to omit it and leave it to the update script to check ;^)
    }
  }
}