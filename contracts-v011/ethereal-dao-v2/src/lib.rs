use scrypto::prelude::*;
use std::ops::DerefMut;
use scrypto::blueprints::consensus_manager::TimePrecision;

// SCHEMA:
// blueprint_call(arg1, arg2, arg3, scrypto_args!(power_zero.mint(1)))
// i.e. calls the init function Arg3 with the given PackageAddress Arg1 and Module Arg2
//      passing in the freshly minted power zero as an argument
//      WARNING: the update script must burn power zero -- and there is nmo way to guarantee that
//               i.e. in essence the update script is TRUSTED with the ENTIRE SYSTEM
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

#[derive(ScryptoSbor)]
pub enum Vote {
  For,
  Against
}

#[derive(ScryptoSbor, Clone)]
struct SubmittedProposal {
  is_active: bool,
  proposal: Proposal,
  when_submitted: Instant,
  who_submitted: ResourceAddress,
  votes_for: Option<ResourceAddress>, // for 3/3 case, positive
  votes_against: Option<ResourceAddress>, // for 2/3 case, negative
}

// FIRST DAO
#[blueprint]
mod dao {
  enable_method_auth! {
    roles {
      zero => updatable_by: [];
    },
    methods {
      set_branch_addrs => restrict_to: [zero];
      get_branch_addrs => PUBLIC;
      add_proposal => PUBLIC;
      vote => PUBLIC;
      finalize_proposal => PUBLIC;
      set_vote_duration => restrict_to: [zero];
      to_nothing => restrict_to: [zero];
    }
  }

  struct Dao {
    power_dao: Vault,
    power_zero: ResourceAddress,

    // alpha, Delta, omega
    souls: (ResourceAddress, ResourceAddress, ResourceAddress),
    branch_addrs: (ComponentAddress, ComponentAddress, ComponentAddress),

    proposals: KeyValueStore<u64, SubmittedProposal>,
    proposal_index: u64,

    // PRAMETERS
    vote_duration: u64
  }

  impl Dao {
    // bang is the dapp definition addr

    pub fn from_something(
      power_dao: Bucket, power_zero: ResourceAddress,
      souls: (ResourceAddress, ResourceAddress, ResourceAddress),
      branch_addrs: (ComponentAddress, ComponentAddress, ComponentAddress),
      bang: ComponentAddress
      ) -> ComponentAddress {

      let branches = branch_addrs.clone();

      let the_zero = power_dao.as_fungible().authorize_with_all(|| 
        ResourceManager::from(power_zero).mint(1)
      );

      let proposals = KeyValueStore::new();
      let proposal_index = 0u64;
      let vote_duration = 84u64; // 7 day long
      
      let dao_addr = Self {
        power_dao: Vault::with_bucket(power_dao),
        power_zero,

        souls,
        branch_addrs,

        proposals,
        proposal_index,
        vote_duration
      }
      .instantiate()
      .prepare_to_globalize(OwnerRole::None)
      .roles(
        roles!(
          zero => rule!(require(power_zero));
        )
      )
      .metadata(
        metadata!(
          roles {
            metadata_setter => rule!(require(power_zero));
            metadata_setter_updater => rule!(deny_all);
            metadata_locker => rule!(deny_all);
            metadata_locker_updater => rule!(deny_all);
          },
          init {
            "dapp_definition" =>
              GlobalAddress::from(bang), updatable;
            "tags" => vec!["ethereal-dao".to_owned(), 
              "dao".to_owned()], updatable;
          }
        )
      )
      .globalize()
      .address();

      the_zero.as_fungible().authorize_with_all(|| {
        let alpha: Global<AnyComponent> = branches.0.into();
        let delta: Global<AnyComponent> = branches.1.into();
        let omega: Global<AnyComponent> = branches.2.into();

        alpha.call_raw::<()>("set_dao_addr", scrypto_args!(dao_addr));
        delta.call_raw::<()>("set_dao_addr", scrypto_args!(dao_addr));
        omega.call_raw::<()>("set_dao_addr", scrypto_args!(dao_addr));
      });

      the_zero.burn();
    
      dao_addr
    }

    pub fn to_nothing(&mut self) -> Bucket {
      self.power_dao.take_all()
    }

    // adds proposal to internal list of vote-able proposals
    pub fn add_proposal(&mut self, proposal: Proposal, user: Proof) -> u64 {
      // can't use 'check' cause it panics lmao
      let soul: ResourceAddress = user.resource_address();

      assert!( self.souls.0 == soul || self.souls.1 == soul || self.souls.2 == soul,
        "wrong proof" );

      // checks the size constraints
      fn check_string(s: &str) {
        // sha256 length
        assert!(s.len() <= 64, "text too long")
      }

      match &proposal {
        Proposal::UpdateBranch(_, s1, s2) => {
          check_string(&s1);
          check_string(&s2);
        },
        Proposal::UpdateSelf(_, s1, s2) => {
          check_string(&s1);
          check_string(&s2);
        }
      }

      self.proposals.insert(
        self.proposal_index,
        SubmittedProposal {
          is_active: true,
          proposal, 
          when_submitted: Clock::current_time_rounded_to_minutes(),
          who_submitted: soul,
          votes_for: None, 
          votes_against: None, 
        }
      );

      self.proposal_index += 1;
      return self.proposal_index - 1;
    }

    pub fn vote(&mut self, vote: Vote, proposal: u64, user: Proof) {
      // is_active, Proposal, when_submitted, who_submitted
      // vote_for, vote_against, vote_abstain
      
      let mut execute_flag = false;

      // yes this is an empty block
      // yes its only purpose is to forcefully fucking drop 'p' after we're done with it
      {
        // ensures proposal actually exist
        let mut p = self.proposals.get_mut(&proposal).unwrap();

        assert!( p.is_active, 
          "vote on finalized proposal"); 

        assert!(
          Clock::current_time_is_strictly_before( 
            p.when_submitted.add_hours(self.vote_duration as i64).expect("days"), 
            TimePrecision::Minute ),
          "vote after closed" );

        let soul = user.resource_address();

        assert!( self.souls.0 == soul || self.souls.1 == soul || self.souls.2 == soul,
          "wrong proof" );

        assert!( soul != p.who_submitted 
          && p.votes_for.map(|x| x != soul).unwrap_or(true)
          && p.votes_against.map(|x| x != soul).unwrap_or(true),
          "double vote");
        
        match p.proposal {
          Proposal::UpdateBranch(_, _, _) => match vote {
            Vote::For => { 
              p.is_active = false;
              p.votes_for = Some(soul);
              execute_flag = true;
            },
            Vote::Against => match p.votes_against {
              None => p.votes_against = Some(soul),
              Some(_) => { 
                p.is_active = false;
                p.votes_against = Some(soul);
              }
            }
          },
          Proposal::UpdateSelf(_, _, _) => match vote {
            Vote::For => match p.votes_for {
              None => p.votes_for = Some(soul),
              Some(_) => {
                p.is_active = false;
                p.votes_for = Some(soul);
                execute_flag = true;
              }
            },
            Vote::Against => {
              p.deref_mut().is_active = false;
              p.votes_against = Some(soul);
            }
          }
        }
      }

      if execute_flag {
        // this fucking line was ripped away from borrow checker's claws
        let p = self.proposals.get(&proposal).unwrap().proposal.clone();
        self._execute_proposal(&p);
      }
    }

    pub fn finalize_proposal(&mut self, proposal: u64) {
      let mut p = self.proposals.get_mut(&proposal).unwrap();

      assert!( p.is_active, 
        "finalize on finalized proposal"); 

      assert!(
        Clock::current_time_is_strictly_after( 
          p.when_submitted.add_hours(self.vote_duration as i64).expect("days"), 
          TimePrecision::Minute ),
        "finalize before closed" );
      
      p.is_active = false;
    }

    fn _execute_proposal(&mut self, proposal: &Proposal) {
      match proposal { 
        Proposal::UpdateBranch(p,m,f) => Self::authorize(
          &mut self.power_dao, || 
          ScryptoVmV1Api::blueprint_call(
            *p, m, f,
            scrypto_args!(ResourceManager::from(self.power_zero).mint(1))
          )
        ),
        Proposal::UpdateSelf(p,m,f) => {
          let power_dao = self.power_dao.take_all();
          Self::authorize(
            &mut self.power_dao, || 
            ScryptoVmV1Api::blueprint_call(
              *p, m, f,
              scrypto_args!(power_dao)
            )
          )
        }
      };
    }

    pub fn get_branch_addrs(&self) -> (ComponentAddress, ComponentAddress, ComponentAddress) {
      self.branch_addrs
    }

    pub fn set_branch_addrs(&mut self, new: (ComponentAddress, ComponentAddress, ComponentAddress)) {
      self.branch_addrs = new;
    }

    // adding so I can change it w/o full migration
    pub fn set_vote_duration(&mut self, new: u64) {
      self.vote_duration = new;
    }

    fn authorize<F: FnOnce() -> O, O>(power: &mut Vault, f: F) -> O {
      let temp = power.as_fungible().take_all();
      let ret = temp.authorize_with_all(|| {
        f()
      });
      power.put(temp.into());
      return ret
    }

  }
}