use scrypto::prelude::*;
use scrypto::blueprints::clock::TimePrecision;

#[derive(ScryptoSbor, PartialEq, Clone)]
pub enum Proposal {
  // gives power zero to it
  // can and may update more than one branch at once
  // (N+1/2)/N consesnsus
  UpdateBranch(PackageAddress, String, String),
  // gives superbadge to it
  // N/N consensus
  UpdateSelf(PackageAddress, String, String)
}

#[blueprint]
mod dao {
  // static-participant multisig 
  // self-governed via 3/3, each participant being a DAO branch
  struct Dao {
    dao_superbadge: Vault,
    souls: ResourceAddress,
    power_zero: ResourceAddress,

    // active proposals
    proposals: Vec<(Proposal, Instant, 
      (Vec<String>, Vec<String>))>,
    proposal_index: u64, // current top index

    vote_duration: u64, // duration of votes in days before allowed to close 
  }

  impl Dao {
    // speaks the word and creates the world
    // returns self addr, alpha addr, Delta addr, omega addr
    pub fn from_nothing() -> (ComponentAddress, ResourceAddress) {
      let dao_superbadge = Vault::with_bucket(ResourceBuilder::new_fungible()
        .mintable(rule!(deny_all), LOCKED)
        .burnable(rule!(deny_all), LOCKED)
        .metadata("name", "EDAO SUPERBADGE")
        .mint_initial_supply(1));

      let power_zero = 
        ResourceBuilder::new_fungible()
          .mintable(
            rule!(require(dao_superbadge.resource_address())), LOCKED)
          .burnable(
            rule!(allow_all), LOCKED)
          // recall for cleaning up old badges
          // not really used, assumed that update script kills itself 
          .recallable(
            rule!(require(dao_superbadge.resource_address())), LOCKED)
          .restrict_withdraw(
            rule!(require(dao_superbadge.resource_address())), LOCKED)
          .restrict_deposit(
            rule!(require(dao_superbadge.resource_address())), LOCKED)
          .metadata("name", "EDAO POWER ZERO")
          .create_with_no_initial_supply();

      let souls = 
        ResourceBuilder::new_string_non_fungible::<()>()
          .mintable(
            rule!(deny_all), LOCKED)
          .burnable(
            rule!(deny_all), LOCKED)
          .recallable(
            rule!(deny_all), LOCKED)
          .restrict_withdraw(
            rule!(require(power_zero)), LOCKED)
          .restrict_deposit(
            rule!(require(power_zero)), LOCKED)
          .metadata("name", "EDAO SOUL")
          .mint_initial_supply([
            ("alpha".try_into().unwrap(), ()),
            ("Delta".try_into().unwrap(), ()),
            ("omega".try_into().unwrap(), ())
          ]);

      let proposals = vec![];
      let proposal_index: u64 = 0;
      let vote_duration: u64 = 7;

      let ca = Self {
        dao_superbadge,
        souls: souls.resource_address(),
        power_zero,
        proposals,
        proposal_index,
        vote_duration
      }.instantiate().globalize();

      // TODO instantiate all 3 houses.
      
      (ca, power_zero)

    }

    // adds proposal and votes in favor of it
    pub fn add_proposal(&mut self, proposal: Proposal, proof: Proof) {
      let house = self._pass_proof(proof);
      let payload = (proposal, Clock::current_time_rounded_to_minutes(), 
        (vec![house.to_owned()], vec![]) );
      
      self.proposals.push(payload);
    }
    

    pub fn vote(&mut self, vote: bool, proposal: Proposal, proposal_idx: u64, proof: Proof) {
      // is eligible to vote
      let house = self._pass_proof(proof);
      self._can_vote(&*house, proposal_idx);

      // is the vote cast appropriately
      let ix = self.proposal_index - proposal_idx;
      let p = &mut self.proposals[ix as usize];
      assert!(proposal == p.0, "incoherence of proposals");
      assert!(
        Clock::current_time_is_strictly_before( 
          p.1.add_days(self.vote_duration as i64).expect("adding days failed"), 
          TimePrecision::Minute ),
        "vote after closed" );

      if vote {
        p.2.0.push(house)
      } else {
        p.2.1.push(house)
      };
    }

    pub fn finalize_proposal(&mut self) {

      let p = self.proposals[0].clone(); // fails if empty
      let voted_in_favor = p.2.0.len();
      let voted = voted_in_favor + p.2.1.len();
      let is_after_close = Clock::current_time_is_strictly_after( 
        p.1.add_days(self.vote_duration as i64).expect("adding days failed"), 
        TimePrecision::Minute);

      match p.0 {
        Proposal::UpdateBranch(_,_,_) if voted_in_favor >= 2 => self._execute_proposal(&p.0),
        Proposal::UpdateSelf(_,_,_) if voted_in_favor == 3 => self._execute_proposal(&p.0),
        _ if is_after_close || voted == 3 => (),
        _ => panic!("vote still ongoing")
      }

      self.proposals.remove(0);
      self.proposal_index += 1;
    }

    // PRIVATE FUNCTIONS 

    fn _pass_proof(&self, proof: Proof) -> String {
      let nft: NonFungible<()> = proof
        .validate_proof(self.souls)
        .expect("wrong resource")
        .non_fungible();

      if let NonFungibleLocalId::String(house) = nft.local_id() {
        return house.value().to_owned()
      } else {
        panic!("incoherence");
      }
    }

    fn _can_vote(&self, house: &str, ix: u64) {
      let (f, a) = &self.proposals[ix as usize].2;
      for v in f {
        if v == house {
          panic!("double vote");
        }
      }
      for v in a {
        if v == house {
          panic!("double vote");
        }
      }
    }

    fn _execute_proposal(&mut self, proposal: &Proposal) {
      match proposal { 
        Proposal::UpdateBranch(p,m,f) => 
          self.dao_superbadge.authorize(||
            borrow_package!(p).call(&*m, &*f, scrypto_args!(
              borrow_resource_manager!(self.power_zero).mint(1)
            ))
          ),
        Proposal::UpdateSelf(p,m,f) => 
          borrow_package!(p).call(&*m, &*f, scrypto_args!(self.dao_superbadge.take_all()))
      }
    }
  }
}