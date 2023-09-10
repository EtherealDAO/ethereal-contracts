use scrypto::prelude::*;
use std::ops::DerefMut;
use scrypto::blueprints::consensus_manager::TimePrecision;

#[derive(ScryptoSbor, NonFungibleData)]
pub struct UserReceipt {
  #[mutable]
  lp_amount: Decimal,
  #[mutable]
  top_voted_index: u64
}

#[derive(ScryptoSbor)]
pub enum Vote {
  For,
  Against,
  Abstain
}

#[derive(ScryptoSbor, Clone)]
pub enum Action {
  TextOnly(String)
}

type Proposal = Vec<Action>;
type Addr = Result<ComponentAddress, (PackageAddress, String)>;

#[derive(ScryptoSbor, Clone)]
struct SubmittedProposal {
  is_active: bool,
  proposal: Proposal,
  when_submitted: Instant,
  who_submitted: NonFungibleLocalId,
  votes_for: Decimal,
  votes_against: Decimal,
  votes_abstaining: Decimal
}

#[blueprint]
mod omega {
  enable_method_auth! {
    roles {
      zero => updatable_by: [];
      delta => updatable_by: [];
    },
    methods {
      to_nothing => restrict_to: [zero];
      new_user => PUBLIC;
      prove_omega => restrict_to: [delta];
      stake => PUBLIC;
      unstake => PUBLIC;
      add_proposal => PUBLIC;
      vote => PUBLIC;
    }
  }
  
  struct Omega {
    dao_addr: ComponentAddress,
    power_omega: Vault,

    // REAL token
    token: Vault,
    token_issued: Decimal,

    // this is V1 so REAL only
    staked_vault: Vault,

    nft_resource: ResourceAddress,

    // is_active, Proposal, when_submitted, who_submitted
    // vote_for, vote_against, vote_abstain
    proposals: KeyValueStore<u64, SubmittedProposal>,
    proposal_index: u64,

    proposal_payment: Decimal,
    vote_duration: u64
  }

  impl Omega {
    pub fn from_nothing(dao_addr: ComponentAddress, power_zero: ResourceAddress,
      power_delta: ResourceAddress, power_omega: Bucket,
      token: Bucket,
      bang: ComponentAddress
    ) -> ComponentAddress {
      
      let staked_resource = token.resource_address();
      let token_issued = 
        ResourceManager::from(staked_resource).total_supply().unwrap() - token.amount();
      let staked_vault = Vault::new(staked_resource);

      let nft_resource = ResourceBuilder::new_ruid_non_fungible::<UserReceipt>(OwnerRole::None)
        .metadata(metadata!(
          roles {
            metadata_setter => rule!(require(power_omega.resource_address()));
            metadata_setter_updater => rule!(deny_all);
            metadata_locker => rule!(deny_all);
            metadata_locker_updater => rule!(deny_all);
          },
          init {
            "name" => "EDAO OmegaV1 UserReceipt".to_owned(), updatable;
            "symbol" => "EO1UR", updatable;
            "key_image_url" => 
              Url::of("https://cdn.discordapp.com/attachments/1092987092864335884/1095874817758081145/logos1.jpeg")
              , updatable;
            "dapp_definitions" =>
              vec!(GlobalAddress::from(bang)), updatable;
          }
        ))
        .mint_roles(mint_roles!(
          minter => rule!(require(power_omega.resource_address()));
          minter_updater => rule!(deny_all);
        ))
        // burns aren't utilized so just keeping it here for the uhh ability
        .burn_roles(burn_roles!(
          burner => rule!(require(power_omega.resource_address()));
          burner_updater => rule!(deny_all);
        ))
        .non_fungible_data_update_roles(non_fungible_data_update_roles!(
          non_fungible_data_updater => rule!(require(power_omega.resource_address()));
          non_fungible_data_updater_updater => rule!(deny_all);
        ))
        .create_with_no_initial_supply()
        .address();

      let proposal_index = 0;
      let proposals = KeyValueStore::new();

      let proposal_payment = dec!(100);
      let vote_duration = 3u64;

      Self {
        dao_addr,
        power_omega: Vault::with_bucket(power_omega),

        nft_resource,
        staked_vault,

        token: Vault::with_bucket(token),
        token_issued,

        proposal_index,
        proposals,

        proposal_payment,
        vote_duration
      }
      .instantiate()
      .prepare_to_globalize(OwnerRole::None)
      .roles(
        roles!(
          delta => rule!(require(power_delta));
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
          }
        )
      )
      .globalize()
      .address()
    }

    pub fn new_user(&mut self) -> Bucket {
      Self::authorize(&mut self.power_omega, || 
        ResourceManager::from(self.nft_resource)
          .mint_ruid_non_fungible(
            UserReceipt { lp_amount: dec!(0), top_voted_index: 0u64 })
      )
    }

    // when adding stake, it doesn't 'vote up' the vote
    // i.e. any votes for pending proposals get lost
    // NOTE: if user had stake AND voted already, then the vote doesn't 'update'
    pub fn stake(&mut self, input: Bucket, user: Proof) {
      // Remember to check/update unclaimed to init token_amount 
      // in case new rewards type was added 

      let rm = ResourceManager::from(self.nft_resource);

      let nft: NonFungible<UserReceipt> = user
        .check(self.nft_resource)
        .as_non_fungible()
        .non_fungible();
      let id = nft.local_id();
      let data = nft.data();

      // impl as only REAL staking for now
      assert!( 
        input.resource_address() == self.token.resource_address(),
        "wrong stake token" );

      // update first due to rust borrow checker
      Self::authorize(&mut self.power_omega, || 
        rm.update_non_fungible_data(
          &id,
          "lp_amount",
          data.lp_amount + input.amount()
        )
      );
      self.staked_vault.put(input);
    }

    pub fn unstake(&mut self, amount: Decimal, user: Proof) -> Bucket {
      let rm = ResourceManager::from(self.nft_resource);

      let nft: NonFungible<UserReceipt> = user
        .check(self.nft_resource)
        .as_non_fungible()
        .non_fungible();
      let id = nft.local_id();
      let data = nft.data();

      // check correct unstake amount
      assert!(
        amount <= data.lp_amount && dec!(0) < amount, 
        "incorrect amount");

      assert!( !self.proposals.get(&data.top_voted_index).unwrap().is_active,
        "unstake before voting finished");

      Self::authorize(&mut self.power_omega, || 
        rm.update_non_fungible_data(
          &id,
          "lp_amount",
          data.lp_amount - amount
        )
      );

      return self.staked_vault.take(amount)
    }

    // adds proposal to internal list of vote-able proposals
    pub fn add_proposal(&mut self, payment: Bucket, proposal: Proposal, user: Proof) {
      assert!( 
        payment.resource_address() == self.token.resource_address() &&
        payment.amount() >= self.proposal_payment, 
        "incorrect payment for adding proposal");

      let nft: NonFungible<UserReceipt> = user
        .check(self.nft_resource)
        .as_non_fungible()
        .non_fungible();
      let id = nft.local_id();

      // checks the size constraints
      self._check_proposal(&proposal);

      let dao: Global<AnyComponent> = self.dao_addr.into();
      let (_, d, _) = dao.call_raw::<(ComponentAddress, ComponentAddress, ComponentAddress)>
        ("get_branch_addrs", scrypto_args!());

      let delta: Global<AnyComponent> = d.into();
      delta.call_raw::<()>("deposit", scrypto_args!(payment));

      self.proposals.insert(
        self.proposal_index,
        SubmittedProposal {
          is_active: true,
          proposal, 
          when_submitted: Clock::current_time_rounded_to_minutes(),
          who_submitted: id.clone(),
          votes_for: dec!(0), 
          votes_against: dec!(0), 
          votes_abstaining: dec!(0)
        }
      );

      self.proposal_index += 1;
    }

    pub fn vote(&mut self, vote: Vote, proposal: u64, user: Proof) {
      // is_active, Proposal, when_submitted, who_submitted
      // vote_for, vote_against, vote_abstain
      assert!( self.proposals.get(&proposal).unwrap().is_active, 
        "vote on finalized proposal"); 

      // ensures proposal actually exist, and therefore user will be locked for a fixed time
      let mut p = self.proposals.get_mut(&proposal).unwrap();

      assert!(
        Clock::current_time_is_strictly_before( 
          p.when_submitted.add_days(self.vote_duration as i64).expect("days"), 
          TimePrecision::Minute ),
        "vote after closed" );

      let rm = ResourceManager::from(self.nft_resource);

      let nft: NonFungible<UserReceipt> = user
        .check(self.nft_resource)
        .as_non_fungible()
        .non_fungible();
      let id = nft.local_id();
      let data = nft.data();

      assert!( data.top_voted_index < proposal, 
        "double vote" );

      // update nft data and execute vote

      Self::authorize(&mut self.power_omega, ||
        rm.update_non_fungible_data(
          &id,
          "top_voted_index",
          proposal
        )
      );
    
      let x = data.lp_amount;
      match vote {
        Vote::For if x > dec!(0) => p.deref_mut().votes_for += x,
        Vote::Against if x > dec!(0) => p.deref_mut().votes_against+= x,
        Vote::Abstain if x > dec!(0) => p.deref_mut().votes_abstaining += x,
        _ => panic!("nonpositive vote")
      };
    }

    pub fn to_nothing(&mut self) {

    }

    // pupeteer omega by delta
    pub fn prove_omega(&self) -> FungibleProof {
      self.power_omega.as_fungible().create_proof_of_amount(dec!(1))
    }

    // internal 

    // checks validity of proposal
    // i.e. that data is correctly formed
    // doesn't check for *existence of components*
    // i.e. IT DOESN'T GUARANTEE IT CAN BE EXECUTED
    // i.e. IF THERE's NO WAY to 'SKIP' execution in case it passes,
    // there's a problem
    fn _check_proposal(&self, prop: &Proposal) {
      // why 13? I felt it appropriate
      assert!(prop.len() <= 13, "proposal too long");

      for action in prop {
        self._check_single_action(action);
      }
    }

    fn _check_single_action(&self, action: &Action) {
      fn check_string(s: &str) {
        // sha256 length
        assert!(s.len() <= 64, "text too long")
      }

      // this *cannot* 
      fn _check_addr(a: &Addr) {
        match a {
          Ok(_) => (),
          Err((_, s)) => check_string(&*s)
        }
      }

      // fn check_edao_proposal(p: &EDaoProposal) {
      //   match p {
      //     EDaoProposal::UpdateBranch(_, s1, s2) => {
      //       check_string(&*s1);
      //       check_string(&*s2);
      //     },
      //     EDaoProposal::UpdateSelf(_, s1, s2) => {
      //       check_string(&*s1);
      //       check_string(&*s2);
      //     }
      //   }
      // }

      // fn check_delta_proposal(p: &DeltaProposal) {
      //   match p {
      //     DeltaProposal::Spend(_, _, a, s) => {
      //       check_addr(&a);
      //       check_string(&*s);
      //     },
      //     DeltaProposal::Issue(_) => (),
      //     DeltaProposal::Whitelist(_) => (),
        
      //     // Omega
      //     DeltaProposal::OmegaVoteEDao(_, _) => (),
        
      //     // EDao actions
      //     DeltaProposal::EDaoAddProposal(p) => check_edao_proposal(&p),
      //     DeltaProposal::EDaoVote(_, _) => ()
      //   }
      // }

      match action {
        Action::TextOnly(s) => check_string(&*s),
        // Protocol actions
        // Action::ProtocolUpdateParams() => (), // TODO
        // Action::ProtocolUpdate() => (), // TODO

        // // EDAO actions
        // Action::EDaoAddProposal(p) => check_edao_proposal(&p),
        // Action::EDaoVote(_, _) => (),

        // // Alpha actions 
        // Action::AlphaChangeParams(_, _, _) => (),

        // // Delta actions 
        // Action::DeltaPuppeteer(p) => check_delta_proposal(&p),
        // Action::DeltaAllowSpend(_, _) => ()
      }
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
