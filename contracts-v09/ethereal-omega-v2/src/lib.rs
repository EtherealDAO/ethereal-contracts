use scrypto::prelude::*;

// TODO reuse in Delta locks
// NOTE: have to keep it in sync with internal state
// but having this helps a lot with offchain querying user state
#[derive(ScryptoSbor, NonFungibleData)]
pub struct UserReceipt {
  #[mutable]
  lp_amount: Decimal,
  #[mutable]
  top_voted_index: u64
}

#[derive(ScryptoSbor)]
pub enum Vote {
  For(Decimal),
  Against(Decimal),
  Abstain(Decimal)
}

external_component! {
  Dao {
    fn get_branch_addrs(&self) -> (ComponentAddress, ComponentAddress, ComponentAddress);
    fn vote(&mut self, vote: bool, proposal: u64, proof: Proof);
  }
}

external_component! {
  Alpha {
    fn get_proposal_ongoing(&self, proposal: u64) -> Option<bool>;
    fn vote(&mut self, vote: Vote, proposal: u64);
  }
}


// V1 -> V2 only difference is allowing 
// unREAL staking / stake locking 
#[blueprint]
mod omega {
  struct Omega {
    dao_addr: ComponentAddress,
    // authority of omega
    power_omega: Vault,
    // checked by delta
    power_delta: ResourceAddress,

    // update auth
    power_zero: ResourceAddress,

    // REAL token
    token: Vault,
    token_issued: Decimal,

    // this is V2 so resource = tri lp
    staked_resource: ResourceAddress,
    staked_vault: Vault,

    // unique user stake nft
    nft_resource: ResourceAddress,

    // KVS vs receipt
    // KVS better as it makes upgrades non-problematic 
    // i.e. can redeem without a soul
    // NftId -> (VoteIndex, Stake Locked)
    vote_locks: KeyValueStore<u128, (u64, Decimal)>

    // parameters
  }

  impl Omega {
    // the reserve has one token it governs
    // returns the badge 
    // doubles down as a from_nothing and from_something
    pub fn from_nothing(
      dao_addr: ComponentAddress,
      power_zero: ResourceAddress,
      power_omega: Bucket, 
      power_delta: ResourceAddress,
      token: Bucket
      ) 
      -> ComponentAddress {
    
      let staked_resource = token.resource_address();
      let token_issued = 
        borrow_resource_manager!(staked_resource).total_supply() - token.amount();
      let staked_vault = Vault::new(staked_resource);
      let nft_resource = ResourceBuilder::new_uuid_non_fungible::<UserReceipt>()
        .mintable(
          rule!(require(power_omega.resource_address())), LOCKED)
        .burnable(
          rule!(allow_all), LOCKED)
        .metadata("name", "EDAO OmegaV1 UserReceipt")
        .create_with_no_initial_supply();
      let vote_locks = KeyValueStore::new();

      let acc_rules = 
        AccessRulesConfig::new()
          .method("to_nothing", rule!(require(power_zero)), LOCKED)
          .method("issue", rule!(require(power_delta)), LOCKED)
          .default(rule!(allow_all), LOCKED);
      
      Self {
        dao_addr,
        power_omega: Vault::with_bucket(power_omega),
        power_delta,
        power_zero,
        token: Vault::with_bucket(token),
        token_issued,
        staked_resource,
        staked_vault,
        nft_resource,
        vote_locks
      }
      .instantiate()
      .globalize_with_access_rules(acc_rules)
    }

    // AuthRule: power_delta
    // issues amount of the token
    // to be called to distribute token to specific initiatives 
    // like team alloc or contributor fund for example
    // delta needs preallow to issue (TODO)
    pub fn issue(&mut self, amount: Decimal) -> Bucket {
      assert!( amount > dec!(0), "nonpositive amount issued");
      self.token_issued += amount;
      self.token.take(amount)
    }

    // AuthRule: power_delta
    // puppeteered only in V1
    pub fn vote_dao(&self, vote: bool, proposal: u64) {
      let proof = self.power_omega.create_proof();
      Dao::at(self.dao_addr).vote(vote, proposal, proof);
    } 

    // AuthRule: power_zero
    // rips the soul + all the tokens out for upgrade purposes
    pub fn to_nothing(&mut self) -> (Bucket, Bucket) {
      (self.power_omega.take_all(), self.token.take_all())
    } 

    // returns issued/available supply
    pub fn issued(&self) -> Decimal {
      self.token_issued
    }
    
    ////// staking

    // anyone can deposit cause free money
    // no rewards in v1

    // no way to burn users
    pub fn new_user(&self) -> Bucket {
      self.power_omega.authorize(|| 
        borrow_resource_manager!(self.nft_resource)
          .mint_uuid_non_fungible::<UserReceipt>( 
            UserReceipt { lp_amount: dec!(0), top_voted_index: 0u64 })
      )
    }

    // when adding stake, it doesn't 'vote up' the vote
    // i.e. any votes for pending proposals get lost
    // NOTE: if user had stake AND voted already, then the vote doesn't 'update'
    pub fn stake(&mut self, input: Bucket, user: Proof) {
      // Remember to check/update unclaimed to init token_amount 
      // in case new rewards type was added 

      let nft: NonFungible<UserReceipt> = user
        .validate_proof(self.nft_resource)
        .expect("wrong resource")
        .non_fungible();
      let data = nft.data();

      // impl as only REAL staking for now
      assert!( 
        input.resource_address() == self.token.resource_address(),
        "wrong stake token" );

      let id = match nft.local_id() {
        NonFungibleLocalId::UUID(uuid) => uuid.value(),
        _ => panic!("resource incoherence")
      };

      // update first due to rust borrow checker
      self.power_omega.authorize(|| 
        borrow_resource_manager!(self.nft_resource)
          .update_non_fungible_data(
            &nft.local_id(),
            "lp_amount",
            data.lp_amount + input.amount()
          )
      );

      if let Some(r) = self.vote_locks.get(&id) {
          self.vote_locks.insert(id, (r.0, r.1 + input.amount()));
          self.staked_vault.put(input);
      } else {
        // no stake
        self.vote_locks.insert(id, (0u64, input.amount()));
        self.staked_vault.put(input);
      }
    }

    pub fn unstake(&mut self, amount: Decimal, user: Proof) -> Bucket {
      let nft: NonFungible<UserReceipt> = user
        .validate_proof(self.nft_resource)
        .expect("wrong resource")
        .non_fungible();
      let data = nft.data();

      let id = match nft.local_id() {
        NonFungibleLocalId::UUID(uuid) => uuid.value(),
        _ => panic!("resource incoherence")
      };

      // check correct unstake amount
      assert!(
        amount <= data.lp_amount && dec!(0) < amount, 
        "incorrect amount");

      if let Some(r) = self.vote_locks.get(&id) {
        let (ix, x) = *r;

        let gpo = Alpha::at(Dao::at(self.dao_addr).get_branch_addrs().0)
          .get_proposal_ongoing(ix);

        match gpo {
          Some(true) => panic!("unstake after vote"),
          _ => {
            self.vote_locks.insert(id, (ix, x - amount));

            self.power_omega.authorize(|| 
              borrow_resource_manager!(self.nft_resource)
                .update_non_fungible_data(
                  &nft.local_id(),
                  "lp_amount",
                  data.lp_amount - amount
                )
            );
  
            return self.staked_vault.take(amount);
          }
        }
      } else {
        // no stake
        panic!("unstake from emptiness");
      }
    }

    // could remove the proposal arg from this + upstream if it gets too big
    // or annoying, it's just a double check against accidental votes when prior finalizes
    pub fn vote(&self, vote: Vote, proposal: u64, user: Proof) {
      let nft: NonFungible<UserReceipt> = user
        .validate_proof(self.nft_resource)
        .expect("wrong resource")
        .non_fungible();
      let data = nft.data();

      let id = match nft.local_id() {
        NonFungibleLocalId::UUID(uuid) => uuid.value(),
        _ => panic!("resource incoherence")
      };

      if let Some(r) = self.vote_locks.get(&id) {
        let (ix, x) = *r; 

        // user forefits the right to vote on prior proposals
        if ix < proposal {
          // not voted, not locked

          let gpo = Alpha::at(Dao::at(self.dao_addr).get_branch_addrs().0)
            .get_proposal_ongoing(ix);

          // check to avoid malicious/mistaken frontends
          assert!( gpo == Some(true), "vote on non existent or finalized proposal");

          // TODO could just remove this as an arg and 
          // just create it here
          match vote {
            Vote::For(n) => assert!(n == x, "wrong vote size"),
            Vote::Against(n) => assert!(n == x, "wrong vote size"),
            Vote::Abstain(n) => assert!(n == x, "wrong vote size"),
          };
          assert!(x == data.lp_amount, "user receipt incoherence");

          // update lock, nft data and execute vote
          self.vote_locks.insert(id, (proposal, x));
          self.power_omega.authorize(|| {
            borrow_resource_manager!(self.nft_resource)
              .update_non_fungible_data(
                &nft.local_id(),
                "top_voted_index",
                proposal
              );

            // if proposal was wrong, it'll explode here
            Alpha::at(Dao::at(self.dao_addr).get_branch_addrs().0)
              .vote(vote, proposal);
          });
        } else {
          // locked 
          panic!("vote after vote");
        }
      } else {
        // no stake
        panic!("vote from emptiness");
      }
    }
  }
}