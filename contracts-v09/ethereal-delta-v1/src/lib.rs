use scrypto::prelude::*;
use scrypto::blueprints::clock::TimePrecision;

#[derive(ScryptoSbor, PartialEq, Clone)]
pub enum EDaoProposal {
  // gives power zero to it
  // can and may update more than one branch at once
  // 2/3 consesnsus
  UpdateBranch(PackageAddress, String, String),
  // gives superbadge to it
  // 3/3 consensus
  UpdateSelf(PackageAddress, String, String)
}

type Addr = Result<ComponentAddress, (PackageAddress, String)>;

#[derive(ScryptoSbor, PartialEq, Clone)]
pub enum Proposal {
  // tbqh we'd need an entire transaction manifest model 
  // here to do it appropriately
  // for a V2 to consider
  // NOTE: doesn't work with NFTs
  Spend(ResourceAddress, Decimal, Addr, String),
  Issue(Decimal), // issues to self, to then spend in next tx
  Whitelist(ResourceAddress),

  // Omega
  OmegaVoteEDao(bool, u64),

  // EDao actions
  EDaoAddProposal(EDaoProposal),
  EDaoVote(bool, u64)
}

external_component! {
  Dao {
    fn get_branch_addrs(&self) -> (ComponentAddress, ComponentAddress, ComponentAddress);
    fn add_proposal(&mut self, proposal: EDaoProposal, proof: Proof);
    fn vote(&mut self, vote: bool, proposal: u64, proof: Proof);
  }
}

external_component! {
  Omega {
    fn issue(&mut self, amount: Decimal) -> Bucket;
    fn vote_dao(&self, vote: bool, proposal: u64);
  }
}

#[blueprint]
mod delta {
  // dynamic-participant multisig executing treasury spending
  // self-governs membership but requires proof of stake
  // ^ both of above will be in DeltaV2, V1 
  // is just puppet of alpha
  struct Delta {
    dao_addr: ComponentAddress,
    power_delta: Vault,
    power_alpha: ResourceAddress,
    power_zero: ResourceAddress,

    member_resource: ResourceAddress,
    // map of member to end of their term limit
    // should do smth like 6? 12 month terms idk?
    members: HashMap<String, Instant>,

    // active proposals
    // number of members assumed small so this method will work
    proposals: Vec<(Proposal, Instant, 
      (Vec<String>, Vec<String>))>,
    proposal_index: u64, // current top index

    vote_duration: u64, // duration of votes in days before allowed to close 
    
    // doubles down as a whitelist and approved spending
    treasury: HashMap<ResourceAddress, (Decimal, Vault)>,
    gov_token: ResourceAddress,
  }

  impl Delta {
    // speaks the word and creates the world
    // returns self addr
    pub fn from_nothing(
      dao_addr: ComponentAddress,
      power_zero: ResourceAddress,
      power_delta: Bucket, 
      power_alpha: ResourceAddress,
      // can pre-allow, will do 0 IMO
      whitelist: Vec<(ResourceAddress, Decimal)>,
      gov_token: ResourceAddress) -> ComponentAddress {

      let member_resource = 
        ResourceBuilder::new_string_non_fungible::<()>()
          .mintable(
            rule!(require(power_delta.resource_address())), LOCKED)
          .burnable(
            rule!(require(power_delta.resource_address())), LOCKED)
          .recallable(
            rule!(deny_all), LOCKED)
          .restrict_withdraw(
            rule!(require(power_delta.resource_address())), LOCKED)
          .restrict_deposit(
            rule!(require(power_delta.resource_address())), LOCKED)
          .metadata("name", "EDAO DELTA MEMBER")
          .create_with_no_initial_supply();
      let members = HashMap::new();

      let proposals = vec![];
      let proposal_index = 0u64;

      let vote_duration = 3u64;

      let mut treasury = HashMap::new();
      for (ra, d) in whitelist {
        treasury.insert(ra, (d, Vault::new(ra)));
      }

      let acc_rules = 
        AccessRulesConfig::new()
          .method("to_nothing", rule!(require(power_zero)), LOCKED)
          .method("puppeteer", rule!(require(power_alpha)), LOCKED)
          .method("allow_spend", rule!(require(power_alpha)), LOCKED)
          .default(rule!(allow_all), LOCKED);

      Self {
        dao_addr,
        power_delta: Vault::with_bucket(power_delta),
        power_alpha,
        power_zero,
        member_resource,
        members,
        proposals,
        proposal_index,
        vote_duration,
        treasury,
        gov_token
      }.instantiate()
      .globalize_with_access_rules(acc_rules)
    }

    // AuthRule: power_zero
    pub fn to_nothing(&mut self) -> (Bucket, Vec<(Decimal, Bucket)>) {
      (
        self.power_delta.take_all(), 
        self.treasury.values_mut()
          .map( |(d, v)| (*d, v.take_all()) )
          .collect()
      )
    }

    pub fn deposit(&mut self, input: Bucket) {
      match self.treasury.get_mut(&input.resource_address()) {
        None => panic!("non whitelist deposit type"),
        Some((_,v)) => v.put(input)
      }
    }

    // governance

    // adds proposal and votes in favor of it
    pub fn add_proposal(&mut self, proposal: Proposal, proof: Proof) {
      // only active members can push
      let house = self._pass_proof(proof);
      let payload = (proposal, Clock::current_time_rounded_to_minutes(), 
        (vec![house.to_owned()], vec![]) );
      
      self.proposals.push(payload);
    }
    
    pub fn vote(&mut self, vote: bool, proposal: u64, proof: Proof) {
      assert!( proposal >= self.proposal_index, "vote on finalized proposal");
      let ix = self.proposal_index - proposal;

      // is eligible to vote
      let house = self._pass_proof(proof);
      self._can_vote(&*house, ix);

      // is the vote cast appropriately
      let p = &mut self.proposals[ix as usize];
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
      let voted_in_favor = Decimal::from(p.2.0.len());
      assert!( 
        Clock::current_time_is_strictly_after( 
          p.1.add_days(self.vote_duration as i64).expect("adding days failed"), 
          TimePrecision::Minute),
        "finalize before vote time over");

      let member_len = Decimal::from(self.members.keys().count());

      // if passed
      if voted_in_favor > member_len/dec!(2) {
        match p.0 {
          Proposal::Spend(res,amnt,_,_) 
            if self.treasury.get(&res).expect("no resource").0 >= amnt
            => self._execute_proposal(&p.0),
          Proposal::Issue(amnt) 
            if self.treasury.get(&self.gov_token).expect("no resource").0 >= amnt
            => self._execute_proposal(&p.0),
          _ => self._execute_proposal(&p.0)
        }
      }

      self.proposals.remove(0);
      self.proposal_index += 1;
    }

    // alpha actions 

    // AuthRule: power_alpha
    // allows alpha alone to get 2/3 in DAO 
    // only if Delta isn't functional 
    pub fn puppeteer(&mut self, proposal: Proposal) {
      // if delta empty, allow anything
      // if delta not empty, allow only spend + whitelist
      let member_len = self.members.keys().count();

      match proposal {
        _ if member_len == 0 => self._execute_proposal(&proposal),
        Proposal::Spend(_,_,_,_) => self._execute_proposal(&proposal),
        Proposal::Whitelist(_) => self._execute_proposal(&proposal),
        _ => () // no effect so tx succeeds 
      }
    }

    // AuthRule: power_alpha
    // ADDS (or subtracts) spend power from Delta
    pub fn allow_spend(&mut self, resource: ResourceAddress, amnt: Decimal) {
      if let Some((x,_)) = self.treasury.get_mut(&resource) {
        *x += amnt;
      }
    }

    // PRIVATE FUNCTIONS 

    fn _pass_proof(&self, proof: Proof) -> String {
      let nft: NonFungible<()> = proof
        .validate_proof(self.member_resource)
        .expect("wrong resource")
        .non_fungible();

      if let NonFungibleLocalId::String(house) = nft.local_id() {
        return house.value().to_owned()
      } else {
        panic!("incoherence");
      }
    }

    fn _can_vote(&self, member: &str, ix: u64) {
      assert!( self.members.get(&member.to_owned()) != None, 
        "not a delta member" );

      let (f, a) = &self.proposals[ix as usize].2;
      for v in f {
        if v == member {
          panic!("double vote");
        }
      }
      for v in a {
        if v == member {
          panic!("double vote");
        }
      }
    }

    // no checks, no whitelist modify, raw
    fn _execute_proposal(&mut self, proposal: &Proposal) {
      match proposal { 
        Proposal::Spend(res,amnt,addr,foo) => {
          // assuming that take > available will crash
          // this does no auth, so should be relatively safe
          // NOTE: this could get indefinitely stuck unable to execute proposals
          // TODO: change to static dispatch instead
          let payment = self.treasury.get_mut(res).expect("no resource").1.take(*amnt);
          match addr {
            Ok(ca) => 
              borrow_component!(ca)
                .call(&*foo, 
                  scrypto_args!(payment)),
            Err((pa,m)) =>
              borrow_package!(pa)
                .call(&*m, &*foo, 
                  scrypto_args!(payment))
          }
        },
        Proposal::Issue(d) => {
          let b = self.power_delta.authorize(|| 
            Omega::at(Dao::at(self.dao_addr).get_branch_addrs().2).issue(*d)
          );
          self.treasury.get_mut(&b.resource_address())
            .expect("missing gov token vault").1.put(b);
        },
        Proposal::Whitelist(r) => {
          if let None = self.treasury.get(r) {
            self.treasury.insert(*r, (dec!(0), Vault::new(*r)));
          }
        },
        Proposal::OmegaVoteEDao(v, ix) =>
          self.power_delta.authorize(|| 
            Omega::at(Dao::at(self.dao_addr).get_branch_addrs().2).vote_dao(*v, *ix)
        ),
        Proposal::EDaoAddProposal(ep) => {
          let pr = self.power_delta.create_proof();
          Dao::at(self.dao_addr).add_proposal(ep.clone(), pr);
        },
        Proposal::EDaoVote(v, ix) => {
          let pr = self.power_delta.create_proof();
          Dao::at(self.dao_addr).vote(*v, *ix, pr);
        },
      }
    }
  }
}