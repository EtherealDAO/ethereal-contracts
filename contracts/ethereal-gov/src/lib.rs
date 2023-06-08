use scrypto::prelude::*;


type Addr = Result<ComponentAddress, (PackageAddress, String)>;

// No, I'm not going to make a lib module to share this datatype
// I will copy it manually.
#[derive(ScryptoSbor)]
pub enum AddDelegationAction {
  // DAO
  AccountDeposit(ComponentAddress),
  DaoUpdate(PackageAddress),

  // GOV
  GovUpdate(ComponentAddress, PackageAddress),

  // STAKING
  StakingInstantiate(PackageAddress),
}

// TODO store all subsystem addresses 
// Q: require gov-update on every subsystem update?

// Ideally this would be a some-sort DSL 
// for calling *arbitrary* functions
// but passing in args as Vec<u8> is too annoying rn
// so I've decided to make it a federated list of f's
// i.e. need Gov-update to add new ones
// REMINDER: use badge whitelist when arbitrary allowed
#[derive(ScryptoSbor, Hash)]
pub enum Action {
  // GOV (Self) Actions 
  ModifyParameters(), // TODO

  // DAO Actions
  AddPower(),
  RemovePower(), // TODO
  AddDelegation(ResourceAddres, AddDelegationAction),
  RemoveDelegation(), // TODO

  // RESERVE Actions
  // Q: should it be handled via Treasury?
}

// EXTERNAL STATIC MODELS

external_component! {
  Dao {
    fn AddPower(&mut self);
    fn RemovePower(&mut self); // TODO
    fn AddDelegation(&mut self, )
  }
}

// Something 
#[derive(ScryptoSbor)]
pub enum Proposal {
  // no-effect poll-only
  // can have off-chain effects due to decisions
  TextOnly(String),
  // regular vote
  ActionSequence(Vec<Action>),
  // unable to be veto'd by guardians, changes gov guardian
  // (assumed rogue guardian, restores one, more can be added later)
  // second argument is the DAO addr (stupid scoping rules)
  // TODO: q: change only by Delta?
  ChangeGovGuardian(Addr, Addr) 
}

#[derive(ScryptoSbor)]
pub enum Vote {
  For(Decimal),
  Against(Decimal),
  Abstain(Decimal)
}

#[blueprint]
mod gov {
  // contract that handles voting upon proposals
  struct Gov {
    // active proposals
    proposals: Vec<(Proposal, Instant, (Decimal, Decimal, Decimal))>,
    index: u64, // current top index

    // stores bages delegated to gov
    // TODO: Recall semantics??
    badges: KeyValueStore<ResourceAddres, Vault>,
    // assumes all Actions need some badge
    // if action doesn't need one, then it clearly doesn't need to be here
    // components are only ones with acc rules so doesn't make sense to have 
    // package-call actions here 
    // instantiation via AddDelegation
    action_record: KeyValueStore<Action, (ComponentAddress, ResourceAddres)>,

    // omega
    gov_guardian_badge: ResourceAddres,

    voting_badge: ResourceAddres,

    // gov parameters

    vote_duration: u64, // duration of votes in days before allowed to close 
    vote_quorum: Decimal, // minimum % participation before considered quorate

  }

  impl Gov {

    pub fn instantiate_gov(
      dao_superbadge: ResourceAddres, // use power 0 for update
      gov_guardian_badge: ResourceAddres, 
      voting_badge: ResourceAddres
      // params
      ) -> ComponentAddress {

    }

    // AuthRule: dao_superbadge
    // 
    pub fn deposit() {

    }

    // adds proposal to internal list of vote-able proposals
    pub fn add_proposal(&mut self, proposal: Proposal) {
      // todo: add some token payment required or smth
    }

    // AuthRule: voting_badge
    // this call is trusted, gov only aggregates the calls
    // proposal and index are used as a double check that the correct call was made
    pub fn vote(&mut self, vote: Vote, proposal: Proposal, proposal_idx: u64) {
      let ix = self.index - proposal_idx;
      let p = self.proposals[ix];
      assert!(proposal == p.0, "incoherence of proposals");
      assert!(
        current_time_is_strictly_after( 
          p.1.add_days(self.vote_duration as i64), 
          TimePrecision::Minute ),
        "vote after closed" );

      let new = match vote {
        For(x) if x > 0 => (p.2.0 + x, p.2.1, p.2.2),
        Against(x) if x > 0 => (p.2.0, p.2.1 + x, p.2.2),
        Abstain(x) if x > 0 => (p.2.0, p.2.1, p.2.2 + x),
        _ => panic!("nonpositive vote")
      };

      self.proposals[ix] = (p.0, p.1, new);
    }

    // either executes or not, depending on result
    // and then removes it out of the internal list 
    // ASSUMPTION: quorum % is accurately counted by reserve 
    // can be broken by giving voting badge to other scripts
    pub fn finalize_proposal(
      &mut self, 
      reserve_addr: ComponentAddress, // quorum req
      ) {

      let p = self.proposals[0]; // fails if empty
      assert!(
        current_time_is_strictly_before( 
          p.1.add_days(self.vote_duration as i64), 
          TimePrecision::Minute ),
        "finalize before closed" );

      // note: in future might want to add custom thresholds
      // for different actions i.e. higher for more important
      // initially everything is a majority win
      match p.2 {
        (y,n,a) if y > n && y+n+a > self.vote_quorum => execute_proposal(p.0),
        _ => info!("proposal rejected")
      };

      self.proposals = self.proposals.remove(0);
      self.index += 1;
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

    // AuthRule: Power 0
    pub fn to_nothing(&mut self) {
      // completely rips apart any current proposals
      // just because 

      // only thing returned here would be the badges
      // i.e. the ones this system cannot move
      // i.e. here we *have to* assume 
      // that they've already been pulled out
      // and this is the last call in a sequence of actions
      // that removed the powers from this Gov
      // and has moved them to the next one already
      // last cleanup being Power 1 removal from this
      // i.e. there is nothing to return
      // i.e. this function is a no-op
      // i.e. the only changes need to be made 
      // in the other components 
      // notably Staking 
    }

    // PRIVATE FUNCTIONS 

    // raw proposal execute logic
    // it better fucking grab the Component/Package into the fucking scope
    fn execute_proposal(&self, prop: Proposal) {
      match prop {
        TextOnly(_) => (),
        ChangeGovGuardian(addr) => {
          match addr {
            _ => () // TODO implement once recall is in
          }
        },
        ActionSequence(v) => {
          for action in v {
            self.execute_single_action(action);
          }
        }
      }
    }

    // eval
    fn execute_single_action(&self, action: Action) {
      let (ca, badge) = *self.power_map.get(action).unwrap();
      badge.authorize(|| 
        match action {
          // GOV Actions
          ModifyParameters() => (), // TODO

          // DAO Actions
          AddPower() => Dao::at(ca).add_power(),
          RemovePower() => Dao::at(ca).remove_power(), // TODO
          AddDelegation(ra, adact) => Dao::at(ca).add_delegation(ra, adact),
          RemoveDelegation() => Dao::at(ca).remove_delegation(), // TODO

          // Staking Actions
        }
      )
    }
  }
}