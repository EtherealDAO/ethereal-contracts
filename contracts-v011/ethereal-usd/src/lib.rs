use scrypto::prelude::*;

// let component: Global<AnyComponent> = Global(ObjectStub::new(ObjectStubHandle::Global(GlobalAddress::from(component_address))));
// let return_value = component.call_raw::<ZygomebFancyReturnType>("method_name", scrypto_args!(param1));

#[derive(NonFungibleData, ScryptoSbor)]
pub struct Flash {
  pub size: Decimal,
  pub isloan: Option<bool> // None -- mint, Some(true) -- XRD, Some(true) -- EXRD 
}

#[blueprint]
mod usd {
  enable_method_auth! {
    roles {
      alpha => updatable_by: [];
      dex => updatable_by: [alpha]; // temporary measure, TODO: alpha + delpoy braider
    },
    methods {
      to_nothing => restrict_to: [alpha]; //todo alpha's power zero
      start_stop => restrict_to: [alpha];
      aa_poke => PUBLIC;
      aa_woke => restrict_to: [dex];
      aa_choke => PUBLIC;
      exrdxrd => PUBLIC;
      flash_loan_start => PUBLIC;
      flash_loan_end => PUBLIC;
      flash_mint_start => PUBLIC;
      flash_mint_end => PUBLIC;
      liquidate => PUBLIC;
      tcr => PUBLIC;
      inject_assets => PUBLIC;
      set_oracle => PUBLIC;
      mock_mint => PUBLIC;
    }
  }

  struct Usd {
    alpha_addr: ComponentAddress,

    power_usd: Vault,

    // eusd
    liability_total: Decimal,
    eusd_resource: ResourceAddress,
    
    // xrd + exrd 
    exrd_vault: Vault,
    xrd_vault: Vault,

    // %-expressed maximum price depeg on open market
    lower_bound: Decimal,
    upper_bound: Decimal,

    // flashing variables, allow only one active flashing in a tx
    // this includes any MP invocation, which means flash self sale is impossible
    flash_resource: ResourceAddress,
    fl_active: bool,
    fm_active: bool,
    flash_fee: Decimal,
    // TODO prevent liquidations when a flash is active

    // fixed exch rate
    mock_oracle: Decimal,

    stopped: bool // TODO make work
  }

  impl Usd {
    pub fn from_nothing(alpha_addr: ComponentAddress, power_alpha: ResourceAddress,
      power_eux: ResourceAddress, power_usd: Bucket, exrd_resource: ResourceAddress, 
      lower_bound: Decimal, upper_bound: Decimal, flash_fee: Decimal, 
      mock_oracle: Decimal
      ) -> (ComponentAddress, ResourceAddress) {

      let flash_resource = ResourceBuilder::new_ruid_non_fungible::<Flash>(OwnerRole::None)
        .metadata(metadata!(
          init {
            "name" => "FLASHFLASHFLASHFLASH".to_owned(), locked;
          }
        ))
        .mint_roles(mint_roles!(
          minter => rule!(require(power_usd.resource_address()));
          minter_updater => rule!(deny_all);
        ))
        .burn_roles(burn_roles!(
          burner => rule!(require(power_usd.resource_address()));
          burner_updater => rule!(deny_all);
        ))
        .deposit_roles(deposit_roles!(
          depositor => rule!(deny_all);
          depositor_updater => rule!(deny_all);
        ))
        .create_with_no_initial_supply()
        .address();

      // TODO: metadata
      let eusd_resource = ResourceBuilder::new_fungible(OwnerRole::None)
        .metadata(metadata!(
          roles {
            metadata_setter => rule!(require(power_usd.resource_address()));
            metadata_setter_updater => rule!(deny_all);
            metadata_locker => rule!(deny_all);
            metadata_locker_updater => rule!(deny_all);
          },
          init {
            "name" => "Ethereal USD".to_owned(), updatable;
            "symbol" => "EUSD".to_owned(), updatable;
          }
        ))
        .mint_roles(mint_roles!(
          minter => rule!(require(power_usd.resource_address()));
          minter_updater => rule!(deny_all);
        ))
        .burn_roles(burn_roles!(
          burner => rule!(require(power_usd.resource_address()));
          burner_updater => rule!(deny_all);
        ))
        .create_with_no_initial_supply()
        .address();

      let a1 = Self {
        alpha_addr,

        power_usd: Vault::with_bucket(power_usd),

        liability_total: dec!(0),
        eusd_resource,
        
        exrd_vault: Vault::new(exrd_resource),
        xrd_vault: Vault::new(XRD),

        lower_bound,
        upper_bound,

        flash_resource,
        fl_active: false,
        fm_active: false,
        flash_fee,

        mock_oracle,

        stopped: false // TODO: need vote to start
      }
      .instantiate()
      .prepare_to_globalize(OwnerRole::None)
      .roles(
        roles!(
          alpha => rule!(require(power_alpha));
          dex => rule!(require(power_eux));
        )
      )
      .globalize()
      .address();

      return (a1, eusd_resource)
    }

    pub fn to_nothing(&mut self) {

    }

    pub fn start_stop(&mut self, input: bool) {
      self.stopped = input;
    }

    pub fn tcr(&self) {
      // TODO call validator
      // and calculate how much XRD is underlying each EXRD 
    }

    pub fn liquidate(&mut self) {
      // todo panic if flashed
      // todo if liq happens, return X% of collateral as payment
    }

    // how much XRD is each EXRD worth 
    // system assumes no time value on unstake 
    pub fn exrdxrd(&self, size: Decimal) -> Decimal {
      // TODO call validator
      size*dec!(1)
    }

    // re: flash mint/loans
    // they cannot trigger MP as that panics
    // so any 'forced liquidation' has to go thru an external market on EXRD/EUSD

    // if res -- exrd, otherwise xrd
    // repayment in either
    pub fn flash_loan_start(&mut self, size: Decimal, res: bool) -> (Bucket, Bucket) {
      assert!(size <= if res { self.exrd_vault.amount() } else { self.xrd_vault.amount() },
        "our size is not size enough"
      );

      let flash = ResourceManager::from(self.flash_resource)
        .mint_ruid_non_fungible(
          Flash {
            size: size*self.flash_fee,
            isloan: Some(res)
          }
        );
      (if res { self.exrd_vault.take(size) } else { self.xrd_vault.take(size) }, flash)
    }

    // allows repayment in either
    // basically allows a very cheap swap between either
    // need to price it like a dex swap
    // i.e. at least 0.1% (ref: aave 0.09%) 
    pub fn flash_loan_end(&mut self, input: Bucket, flash: Bucket) {
      assert!(flash.resource_address() == self.flash_resource,
        "not flash");
      assert!(!self.fl_active, 
        "twice flash loaned");

      let data: Flash = flash.as_non_fungible().non_fungible().data();
      
      match data.isloan {
        Some(true) => {
          if input.resource_address() == self.exrd_vault.resource_address() {
            assert!(input.amount() >= data.size,
              "insufficient size");

            self.exrd_vault.put(input);
          } else {
            // TODO call exchange rate
            self.xrd_vault.put(input);
          }
        }
        Some(false) => {
          if input.resource_address() == self.exrd_vault.resource_address() {   
            // TODO call exchange rate       
            self.exrd_vault.put(input);
          } else {
            assert!(input.amount() >= data.size,
              "insufficient size");

            self.xrd_vault.put(input);
          }
        }
        None => panic!("wrong resource type")
      };

      Self::authorize(&mut self.power_usd, || {
        ResourceManager::from(self.flash_resource).burn(flash)
      })
    }

    // TODO: impose limit on the size?
    // what could go wrong? lmao
    pub fn flash_mint_start(&mut self, size: Decimal) -> (Bucket, Bucket) {
      assert!(!self.fm_active, 
        "twice flash minted");

      Self::authorize(&mut self.power_usd, || {
        let flash = ResourceManager::from(self.flash_resource)
          .mint_ruid_non_fungible(
            Flash {
              size: size*self.flash_fee,
              isloan: None
          });

        self.liability_total += size;
        self.fm_active = true;

        (ResourceManager::from(self.eusd_resource).mint(size), flash)
      })
    }

    pub fn flash_mint_end(&mut self, input: Bucket, flash: Bucket) {
      assert!(flash.resource_address() == self.flash_resource,
        "not flash");

      let data: Flash = flash.as_non_fungible().non_fungible().data();
      assert!(input.amount() >= data.size,
        "insufficient size");
      assert!(input.resource_address() == self.eusd_resource,
        "wrong resource");

      match data.isloan {
        None => (),
        _ => panic!("wrong flash type")
      };

      self.liability_total -= input.amount();
      self.fm_active = false;

      Self::authorize(&mut self.power_usd, || {
        ResourceManager::from(self.eusd_resource).burn(input);
        ResourceManager::from(self.flash_resource).burn(flash);
      });
    }

    // check if aa is necessary
    // contains all the mandatory pegging logic
    // for v2, to maybe AMO-ize that
    // spot is EUSD/EXRD, oracle is EUSD/XRD -- needs to rescale
    pub fn aa_poke(&mut self, spot: Decimal) -> Option<(Decimal, Decimal, bool)> {
      info!("aa_poke IN"); 
      // todo panic if flashed
      // todo automatically add LP from profits + treasury REAL
      // todo TCR checks
      // TODO share some of the profit with ECDPs 
      // todo send profits to treasury
      // todo spot gives EUSD/EXRD, we have exchr EUSD/XRD (todo lookup XRD/EXRD)
      if spot > self.mock_oracle * self.upper_bound {
        Some((self.mock_oracle * self.upper_bound, self.mock_oracle, true))
      } else if spot < self.mock_oracle * self.lower_bound {
        Some((self.mock_oracle * self.lower_bound, self.mock_oracle, false))
      } else { // todo rescale the oracle to EXRD units 
        None
      }
    }

    // execute aa
    pub fn aa_woke(&mut self, size: Decimal, direction: bool) -> Bucket {
      info!("aa_woke IN"); 
      if direction {
        Self::authorize(&mut self.power_usd, || {
          // TODO tcr check, mint only as much as can
          self.liability_total += size;
          ResourceManager::from(self.eusd_resource).mint(size)
        })
      } else {
        Self::authorize(&mut self.power_usd, || {
          if self.exrd_vault.amount() <= size {
            panic!("todo")
            // TODO: pull liq from XRD and change into EXRD 
            // (and if that's not enough, protocol is broke lol)
          } else {
            self.exrd_vault.take(size)
          }
        })
      }
    }
    
    // get aa profit, in LP
    // input is EUXLP -> Treasury to be changed into TLP
    // remainder is of type dep on direction -- incoherence panics
    pub fn aa_choke(&mut self, ret: Bucket, profit: Bucket, direction: bool) {
      info!("aa_choke IN"); 

      let alpha: Global<AnyComponent> = self.alpha_addr.into();
      if direction {
        self.exrd_vault.put(ret);

        // todo: authorize question on alpha
        alpha.call_raw::<()>("aa_rope", scrypto_args!(profit));
      } else {
        self.liability_total -= ret.amount();
        Self::authorize(&mut self.power_usd, || {
          ResourceManager::from(self.eusd_resource).burn(ret)
        });

        // todo: authorize question on alpha
        alpha.call_raw::<()>("aa_rope", scrypto_args!(profit));
      }
    }

    // internal 

    fn authorize<F: FnOnce() -> O, O>(power: &mut Vault, f: F) -> O {
      let temp = power.as_fungible().take_all();
      let ret = temp.authorize_with_all(|| {
        f()
      });
      power.put(temp.into());
      return ret
    }

    // mock functions

    pub fn inject_assets(&mut self, ass: Bucket) {
      self.exrd_vault.put(ass);
    }

    pub fn mock_mint(&mut self, size: Decimal) -> Bucket {
      Self::authorize(&mut self.power_usd, || 
        ResourceManager::from(self.eusd_resource).mint(size))
    }

    pub fn set_oracle(&mut self, exch: Decimal) {
      self.mock_oracle = exch;
    }
  }
}