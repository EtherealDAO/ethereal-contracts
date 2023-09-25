use scrypto::prelude::*;
use std::ops::DerefMut;

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

#[derive(ScryptoSbor, PartialEq, Clone)]
pub enum EDaoProposal {
  UpdateBranch(PackageAddress, String, String),
  UpdateSelf(PackageAddress, String, String)
}

#[derive(ScryptoSbor, Clone)]
pub enum EDaoVote {
  For,
  Against
}

#[derive(ScryptoSbor, Clone)]
pub enum Action {
  TextOnly(String),

  // EDAO actions
  EDaoAddProposal(EDaoProposal),
  EDaoVote(EDaoVote, u64), 

  DeltaWithdraw(Addr, String, ResourceAddress, Decimal),

  OmegaIssue(Addr, String, Decimal),
  OmegaAddAAReal(Decimal),

  // Protocol (Parameter) Actions
  EUSDChangeParam(u64, Decimal), // 0-based, change a single number

  // first eusd -> first eusd/exrd deposit -> first real/euxlp deposit
  // RA is intended to be the EXRD address
  AllFirstDaisyChain(ResourceAddress),

  // For starting it up, would be not very useful for stopping
  // unless as last action in the gov update pipeline
  StartStopAll(bool),

  // For propocol upgrades, and things not included in the "safe" gov building blocks
  // Schema: calls that package with a single argument of either power_azero or power_zero
  ManualWithPZeroAuth(PackageAddress, String, String),
  ManualWithPAZeroAuth(PackageAddress, String, String)
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

#[derive(ScryptoSbor, ScryptoEvent)]
struct ProposalSubmittedEvent {
  proposal: u64,
  who_submitted: NonFungibleLocalId
}

#[derive(ScryptoSbor, ScryptoEvent)]
struct ProposalFinalizedEvent {
  proposal: u64,
  result: bool
}

#[blueprint]
#[types(UserReceipt, Proposal, Addr, Action, Vote, SubmittedProposal, u64)]
#[events(ProposalSubmittedEvent, ProposalFinalizedEvent)]
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
      set_dao_addr => restrict_to: [zero];
      finalize_proposal => PUBLIC;
    }
  }
  
  struct Omega {
    dao_addr: ComponentAddress,
    power_omega: Vault,

    // REAL token
    token: Vault,

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
      let staked_vault = Vault::new(staked_resource);

      let nft_resource = 
        ResourceBuilder::new_ruid_non_fungible_with_registered_type::<UserReceipt>(OwnerRole::None)
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
            "tags" => vec!["ethereal-dao".to_owned(), "staking".to_owned(), "badge".to_owned()], updatable;
            "info_url" => Url::of("https://ethereal.systems"), updatable;
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

      let proposal_index = 1;
      let proposals = KeyValueStore::new_with_registered_type();

      let proposal_payment = dec!(100);
      let vote_duration = 1u64; // TODO: 36u6 / 3 days on release

      Self {
        dao_addr,
        power_omega: Vault::with_bucket(power_omega),

        nft_resource,
        staked_vault,

        token: Vault::with_bucket(token),

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
            "tags" => vec!["ethereal-dao".to_owned(), 
              "omega".to_owned()], updatable;
          }
        )
      )
      .globalize()
      .address()
    }

    pub fn to_nothing(&mut self) -> (Bucket, Bucket) {
      (self.power_omega.take_all(), self.token.take_all())
    }

    pub fn new_user(&mut self) -> Bucket {
      self.power_omega.as_fungible().authorize_with_amount(dec!(1), ||  
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
      self.power_omega.as_fungible().authorize_with_amount(dec!(1), ||  
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

      self.power_omega.as_fungible().authorize_with_amount(dec!(1), || 
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
          proposal: proposal.clone(), 
          when_submitted: Clock::current_time_rounded_to_minutes(),
          who_submitted: id.clone(),
          votes_for: dec!(0), 
          votes_against: dec!(0), 
          votes_abstaining: dec!(0)
        }
      );

      Runtime::emit_event( 
        ProposalSubmittedEvent { proposal: self.proposal_index, who_submitted: id.clone() } );

      self.proposal_index += 1;
    }

    pub fn vote(&mut self, vote: Vote, proposal: u64, user: Proof) {
      // ensures proposal actually exist, and therefore user will be locked for a fixed time
      let mut p = self.proposals.get_mut(&proposal).unwrap();

      // is_active, Proposal, when_submitted, who_submitted
      // vote_for, vote_against, vote_abstain
      assert!( p.is_active, 
        "vote on finalized proposal"); 

      assert!(
        Clock::current_time_is_strictly_before( 
          p.when_submitted.add_hours(self.vote_duration as i64).expect("days"), 
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

      self.power_omega.as_fungible().authorize_with_amount(dec!(1), || 
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

    pub fn finalize_proposal(&mut self, proposal: u64) {
      let mut execute_flag = None;
      {
        let mut p = self.proposals.get_mut(&proposal).unwrap();

        assert!( p.is_active, 
          "finalize on finalized proposal"); 

        assert!(
          Clock::current_time_is_strictly_after( 
            p.when_submitted.add_hours(self.vote_duration as i64).expect("days"), 
            TimePrecision::Minute ),
          "finalize before closed" );

        p.is_active = false;
        
        // no quorum yet
        if p.votes_for - p.votes_against > dec!(0) {
          execute_flag = Some(p.proposal.clone());
        } 
      }

      if let Some(p) = execute_flag {
        self._execute_proposal(&p);
        Runtime::emit_event( ProposalFinalizedEvent { proposal, result: true } );
      } else {
        Runtime::emit_event( ProposalFinalizedEvent { proposal, result: false } );
      }
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
      fn check_addr(a: &Addr) {
        match a {
          Ok(_) => (),
          Err((_, s)) => check_string(&*s)
        }
      }

      fn check_edao_proposal(p: &EDaoProposal) {
        match p {
          EDaoProposal::UpdateBranch(_, s1, s2) => {
            check_string(&*s1);
            check_string(&*s2);
          },
          EDaoProposal::UpdateSelf(_, s1, s2) => {
            check_string(&*s1);
            check_string(&*s2);
          }
        }
      }

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

        // EDAO actions
        Action::EDaoAddProposal(p) => check_edao_proposal(&p),
        Action::EDaoVote(_, _) => (),

        // Protocol Param actions
        Action::EUSDChangeParam(i,_) => assert!( *i < 6u64, "out of bounds" ),

        // StartStop
        Action::StartStopAll(_) => (),

        // For propocol upgrades, and things not included in the "safe" gov building blocks
        Action::ManualWithPZeroAuth(_, s1, s2) => { check_string(&*s1); check_string(&*s2) },
        Action::ManualWithPAZeroAuth(_, s1, s2) => { check_string(&*s1); check_string(&*s2) },

        // // Alpha actions 
        // Action::AlphaChangeParams(_, _, _) => (),

        // Delta actions 
        Action::DeltaWithdraw(addr, s, _, _) => { check_addr(&addr); check_string(&s) },

        // Omega actions
        Action::OmegaIssue(addr, s, _) => { check_addr(&addr); check_string(&s) },
        Action::OmegaAddAAReal(_) => (),

        // Setup Actions
        Action::AllFirstDaisyChain(_) => ()
      }
    }

    // raw proposal execute logic
    // it better fucking grab the Component/Package into the fucking scope
    fn _execute_proposal(&mut self, prop: &Proposal) {
      for action in prop {
        self._execute_single_action(action);
      }
    }

    // eval
    fn _execute_single_action(&mut self, action: &Action) {
      match action {
        Action::TextOnly(_) => (),
        // // Protocol actions
        // Action::ProtocolUpdateParams() => (), // TODO
        // Action::ProtocolUpdate() => (), // TODO

        // EDAO actions
        Action::EDaoAddProposal(edao_proposal) => {
          let dao: Global<AnyComponent> = self.dao_addr.into();
          dao.call_raw::<()>("add_proposal", scrypto_args!(edao_proposal, self.prove_omega()))
        },
        Action::EDaoVote(vote, proposal) => {
          let dao: Global<AnyComponent> = self.dao_addr.into();
          dao.call_raw::<()>("vote", scrypto_args!(vote, proposal, self.prove_omega()))
        }, 

        // Protocol Param actions
        Action::EUSDChangeParam(ix, new) => {
          let dao: Global<AnyComponent> = self.dao_addr.into();
          let (a, _, _) = dao.call_raw::<(ComponentAddress, ComponentAddress, ComponentAddress)>
            ("get_branch_addrs", scrypto_args!());
          
          let alpha: Global<AnyComponent> = a.into();
          let (u, _, _) = alpha.call_raw::<(ComponentAddress, ComponentAddress, ComponentAddress)>
            ("get_app_addrs", scrypto_args!());

          let usd: Global<AnyComponent> = u.into();
          let mut params = usd.call_raw::<(Decimal, Decimal, Decimal, Decimal, Decimal, Decimal)>
            ("get_params", scrypto_args!());

          // :^)
          match ix {
            0 => params.0 = *new,
            1 => params.1 = *new,
            2 => params.2 = *new,
            3 => params.3 = *new,
            4 => params.4 = *new,
            5 => params.5 = *new,
            _ => panic!()
          }

          self.power_omega.as_fungible().authorize_with_amount(dec!(1), || {
            let a0 = alpha.call_raw::<Bucket>("make_azero", scrypto_args!());
            a0.as_fungible().authorize_with_all( ||
              usd.call_raw::<()>("set_params", scrypto_args!(params))
            );
            a0.burn();
          });
        },

        // StartStop
        Action::StartStopAll(startstop) => {
          let dao: Global<AnyComponent> = self.dao_addr.into();
          let (a, _, _) = dao.call_raw::<(ComponentAddress, ComponentAddress, ComponentAddress)>
            ("get_branch_addrs", scrypto_args!());
          
          let alpha: Global<AnyComponent> = a.into();
          let (u, e, t) = alpha.call_raw::<(ComponentAddress, ComponentAddress, ComponentAddress)>
            ("get_app_addrs", scrypto_args!());

          self.power_omega.as_fungible().authorize_with_amount(dec!(1), || {
            let usd: Global<AnyComponent> = u.into();
            let eux: Global<AnyComponent> = e.into();
            let tri: Global<AnyComponent> = t.into();

            let a0 = alpha.call_raw::<Bucket>("make_azero", scrypto_args!());
            a0.as_fungible().authorize_with_all( || {
              usd.call_raw::<()>("start_stop", scrypto_args!(startstop));
              eux.call_raw::<()>("start_stop", scrypto_args!(startstop));
              tri.call_raw::<()>("start_stop", scrypto_args!(startstop));
            });
            a0.burn();
          });
        },

        // For propocol upgrades, and things not included in the "safe" gov building blocks
        // note: via power zero, it can effectively alsso rip dao badge out i.e. total update
        Action::ManualWithPZeroAuth(pa, s1, s2) => { 
          let dao: Global<AnyComponent> = self.dao_addr.into();
          let (a, _, _) = dao.call_raw::<(ComponentAddress, ComponentAddress, ComponentAddress)>
            ("get_branch_addrs", scrypto_args!());
          
          let alpha: Global<AnyComponent> = a.into();

          let edao_proposal = EDaoProposal::UpdateBranch(*pa, s1.clone(), s2.clone());
          let ix = dao.call_raw::<()>("add_proposal", scrypto_args!(edao_proposal, self.prove_omega()));
          let p = self.power_omega.as_fungible().authorize_with_amount(dec!(1), || 
            alpha.call_raw::<FungibleProof>("prove_alpha", scrypto_args!()));
          dao.call_raw::<()>("vote", scrypto_args!(EDaoVote::For, ix, p));
          // 2/3 is reached so it executes the proposal function, moves power zero to it
          // will (likely) be a 2 step process, and if it's an upgrade, 
          // likely best to also stop system after this call
        },

        Action::ManualWithPAZeroAuth(pm, s1, s2) => { 
          let dao: Global<AnyComponent> = self.dao_addr.into();
          let (a, _, _) = dao.call_raw::<(ComponentAddress, ComponentAddress, ComponentAddress)>
            ("get_branch_addrs", scrypto_args!());
          
          let alpha: Global<AnyComponent> = a.into();
          let a0 = alpha.call_raw::<FungibleProof>("make_azero", scrypto_args!());
          ScryptoVmV1Api::blueprint_call(
            *pm,
            s1,
            s2,
            scrypto_args!(a0)
          );
        },

        // Delta actions 
        Action::DeltaWithdraw(addr, s, ra, size) => {
          let dao: Global<AnyComponent> = self.dao_addr.into();
          let (a, d, _) = dao.call_raw::<(ComponentAddress, ComponentAddress, ComponentAddress)>
            ("get_branch_addrs", scrypto_args!());

          let alpha: Global<AnyComponent> = a.into();
          let delta: Global<AnyComponent> = d.into();

          let p = self.power_omega.as_fungible().authorize_with_amount(dec!(1), || 
            alpha.call_raw::<FungibleProof>("prove_alpha", scrypto_args!()));
          let ret = p.authorize(|| 
            delta.call_raw::<Bucket>("withdraw", scrypto_args!(ra, size)));

          Self::call_addr(addr, &*s, scrypto_args!(ret));
        },

        // Omega actions
        Action::OmegaIssue(addr, s, size) => 
          Self::call_addr(addr, &*s, scrypto_args!(self.token.take(*size))),
        Action::OmegaAddAAReal(size) => {
          let dao: Global<AnyComponent> = self.dao_addr.into();
          let (_, d, _) = dao.call_raw::<(ComponentAddress, ComponentAddress, ComponentAddress)>
            ("get_branch_addrs", scrypto_args!());

          let delta: Global<AnyComponent> = d.into();

          delta.call_raw::<()>("add_to_aa", scrypto_args!(self.token.take(*size)));
        },

        // Setup Actions
        Action::AllFirstDaisyChain(exrd) => {
          let dao: Global<AnyComponent> = self.dao_addr.into();
          let (a, d, _) = dao.call_raw::<(ComponentAddress, ComponentAddress, ComponentAddress)>
            ("get_branch_addrs", scrypto_args!());

          let alpha: Global<AnyComponent> = a.into();
          let (u, e, t) = alpha.call_raw::<(ComponentAddress, ComponentAddress, ComponentAddress)>
            ("get_app_addrs", scrypto_args!());

          let delta: Global<AnyComponent> = d.into();
          let pa = self.power_omega.as_fungible().authorize_with_amount(dec!(1), || 
            alpha.call_raw::<FungibleProof>("prove_alpha", scrypto_args!()));
          
          // 50k for 'backing' the 777 EUSD, 17k to be paired with EUSD
          // the numbers are mostly there to be 'close enough'
          // but otherwise statically well telegraphed
          let mut exrd = pa.authorize(|| 
            delta.call_raw::<Bucket>("withdraw", scrypto_args!(exrd, dec!("67000"))));
          
          let a0 = self.power_omega.as_fungible().authorize_with_amount(dec!(1), || 
            alpha.call_raw::<Bucket>("make_azero", scrypto_args!()));
          
          a0.as_fungible().authorize_with_all(|| {
            let usd: Global<AnyComponent> = u.into();
            let eux: Global<AnyComponent> = e.into();
            let tri: Global<AnyComponent> = t.into();

            let (ecdp, eusd) = usd.call_raw::<(Bucket, Bucket)>("first_ecdp", 
              scrypto_args!(exrd.take(dec!("50000"))));
            
            let (euxlp, orem) = eux.call_raw::<(Bucket, Option<Bucket>)>("first_deposit", 
              scrypto_args!(eusd, exrd));
            
            // TODO put number after price discovery happens
            let real = self.token.take(dec!("3.50")); 
            let (etlp, orem2) = tri.call_raw::<(Bucket, Option<Bucket>)>("first_deposit",
              scrypto_args!(real, euxlp));

            delta.call_raw::<()>("deposit", scrypto_args!(etlp));
            delta.call_raw::<()>("deposit", scrypto_args!(ecdp));
            if let Some(rem) = orem {
              delta.call_raw::<()>("deposit", scrypto_args!(rem));
            };
            if let Some(rem2) = orem2 {
              delta.call_raw::<()>("deposit", scrypto_args!(rem2));
            };
          });
          a0.burn();
        }
      }
    }

    pub fn set_dao_addr(&mut self, new: ComponentAddress) {
      self.dao_addr = new;
    }

    // type Addr = Result<ComponentAddress, (PackageAddress, String)>;
    fn call_addr(addr: &Addr, s: &str, args: Vec<u8>) {
      match addr {
        Ok(ca) => {
          let c: Global<AnyComponent> = (*ca).into();
          c.call_raw::<()>(s, args);
        },
        Err((pa, md)) => {
          ScryptoVmV1Api::blueprint_call(*pa, md, &*s, args);
        }
      }
    }
  }
}
