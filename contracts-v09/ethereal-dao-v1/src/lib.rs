use scrypto::prelude::*;
use scrypto::blueprints::clock::TimePrecision;

type BranchAddrs = (ComponentAddress, ComponentAddress, ComponentAddress);

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

external_blueprint! {
  Alpha {
    fn from_nothing(
      dao_addr: ComponentAddress,
      power_zero: ResourceAddress,
      power_alpha: Bucket,
      power_omega: ResourceAddress,

      gov_token: ResourceAddress,
      alpha_vote_duration: u64,
      alpha_vote_quorum: Option<Decimal>,
      alpha_proposal_payment: Decimal
      ) -> ComponentAddress;
  }
}

external_blueprint! {
  Delta {
    fn from_nothing(
      dao_addr: ComponentAddress,
      power_zero: ResourceAddress,
      power_delta: Bucket, 
      power_alpha: ResourceAddress,

      whitelist: Vec<(ResourceAddress, Decimal)>,
      gov_token: ResourceAddress
      ) -> ComponentAddress;
  }
}

external_blueprint! {
  Omega {
    fn from_nothing(
      dao_addr: ComponentAddress,
      power_zero: ResourceAddress,
      power_omega: Bucket, 
      power_delta: ResourceAddress,

      token: Bucket
      ) -> ComponentAddress;
  }
}

// yes, lol
external_component! {
  EDao {
    fn set_branch_addrs(&mut self, new: BranchAddrs);
  }
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
    proposals: KeyValueStore<u64, Option<(Proposal, Instant, 
      (Vec<String>, Vec<String>))> >,
    proposal_index: u64, // current top index

    vote_duration: u64, // duration of votes in days before allowed to close 

    branch_addrs: (ComponentAddress, ComponentAddress, ComponentAddress)
  }

  impl Dao {
    // speaks the word and creates the world
    // returns self addr, alpha addr, Delta addr, omega addr
    pub fn from_nothing(
      package_alpha: PackageAddress,
      alpha_vote_duration: u64,
      alpha_vote_quorum: Option<Decimal>,
      alpha_proposal_payment: Decimal,

      package_delta: PackageAddress,

      package_omega: PackageAddress,
      token: Bucket,

      xrd: ResourceAddress,
    ) -> ComponentAddress {
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
          .mint_initial_supply(1);

      let mut souls = 
        ResourceBuilder::new_string_non_fungible::<()>()
          .mintable(
            rule!(deny_all), LOCKED)
          .burnable(
            rule!(deny_all), LOCKED)
          .metadata("name", "EDAO SOUL")
          .mint_initial_supply([
            ("alpha".try_into().unwrap(), ()),
            ("Delta".try_into().unwrap(), ()),
            ("omega".try_into().unwrap(), ())
          ]);

      let proposals = KeyValueStore::new();
      let proposal_index: u64 = 0;
      let vote_duration: u64 = 7;

      let acc_rules = 
        AccessRulesConfig::new()
          .method("set_branch_addrs", 
            rule!(require(power_zero.resource_address())), LOCKED)
          .default(rule!(allow_all), LOCKED);

      let bang = 
        ComponentAddress::virtual_identity_from_public_key(
          &PublicKey::EcdsaSecp256k1(
            EcdsaSecp256k1PublicKey::from_str(
              "0345495dce6516c31862d36d1d0b254fad29ab016b6d972ebac1dd3902a41b0f9b").unwrap()
          )
        );

      let dao_addr = Self {
        dao_superbadge,
        souls: souls.resource_address(),
        power_zero: power_zero.resource_address(),
        proposals,
        proposal_index,
        vote_duration,
        branch_addrs: (bang, bang, bang)
      }
      .instantiate()
      .globalize_with_access_rules(acc_rules);

      let power_alpha = souls.take_non_fungible(
        &NonFungibleLocalId::string("alpha").unwrap());
      let alpha_resource = power_alpha.resource_address();
      
      let power_delta = souls.take_non_fungible(
        &NonFungibleLocalId::string("Delta").unwrap());
      let delta_resource = power_delta.resource_address();

      let power_omega = souls.take_non_fungible(
        &NonFungibleLocalId::string("omega").unwrap());

      // yes this is necessary
      souls.burn();

      let alpha_addr = Alpha::at(package_alpha, "Alpha")
        .from_nothing(
          dao_addr,
          power_zero.resource_address(),
          power_alpha,
          power_omega.resource_address(),

          token.resource_address(),
          alpha_vote_duration,
          alpha_vote_quorum,
          alpha_proposal_payment
        );

      let delta_addr = Delta::at(package_delta, "Delta")
        .from_nothing(
          dao_addr,
          power_zero.resource_address(),
          power_delta,
          alpha_resource,
          vec![(xrd, dec!(0)), (token.resource_address(), dec!(0))],
          token.resource_address()
        );

      let omega_addr = Omega::at(package_omega, "Omega")
        .from_nothing(
          dao_addr,
          power_zero.resource_address(),
          power_omega,
          delta_resource,
          token
        );

      power_zero.authorize(|| 
        EDao::at(dao_addr).set_branch_addrs((alpha_addr, delta_addr, omega_addr))
      );
      power_zero.burn();

      dao_addr
    }

    // adds proposal and votes in favor of it
    pub fn add_proposal(&mut self, proposal: Proposal, proof: Proof) {
      let house = self._pass_proof(proof);
      let payload = (proposal, Clock::current_time_rounded_to_minutes(), 
        (vec![house.to_owned()], vec![]) );
      
      self.proposals.insert(self.proposal_index, Some(payload));
      self.proposal_index += 1;
    }

    // Some(true) - exists and ongoing
    // Some(false) - exists but finalized
    // Nothing - never existed
    pub fn get_proposal_ongoing(&self, proposal: u64) -> Option<bool> {
      self.proposals.get(&proposal).map(|x| x.is_some())
    }
    
    pub fn vote(&mut self, vote: bool, proposal: u64, proof: Proof) {
      // is eligible to vote
      let house = self._pass_proof(proof);
      self._can_vote(&*house, proposal);

      // is the vote cast appropriately
      let mut p = self.proposals.get_mut(&proposal)
        .expect("non existent proposal");

      assert!(
        Clock::current_time_is_strictly_before( 
          p.as_ref().unwrap().1.add_days(self.vote_duration as i64).expect("adding days failed"), 
          TimePrecision::Minute ),
        "vote after closed" );

      if vote {
        p.as_mut().unwrap().2.0.push(house)
      } else {
        p.as_mut().unwrap().2.1.push(house)
      };
    }

    pub fn finalize_proposal(&mut self, proposal: u64) {
      let mut p = self.proposals.get_mut(&proposal)
        .expect("proposal does not exit");

      let voted_in_favor = p.clone().unwrap().2.0.len();
      let voted = voted_in_favor + p.clone().unwrap().2.1.len();
      let is_after_close = Clock::current_time_is_strictly_after( 
        p.as_ref().unwrap().1.add_days(self.vote_duration as i64).expect("adding days failed"), 
        TimePrecision::Minute);

      match p.as_ref() {
        Some((p @ Proposal::UpdateBranch(_,_,_), _, _)) 
          if voted_in_favor >= 2 => self._execute_proposal(&p),

        Some((p @ Proposal::UpdateSelf(_,_,_), _, _)) 
          if voted_in_favor == 3 => self._execute_proposal(&p),

        _ if is_after_close || voted == 3 => (),
        _ => panic!("vote still ongoing")
      };

      *p = None;
    }

    pub fn get_branch_addrs(&self) -> BranchAddrs {
      self.branch_addrs
    }

    // AuthRule: power_zero
    pub fn set_branch_addrs(&mut self, new: BranchAddrs) {
      self.branch_addrs = new;
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
      let b = &self.proposals.get(&ix)
        .expect("proposal doesn't exit");
        
      let (f, a) = &b
        .as_ref()
        .expect("proposal finalized").2;

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