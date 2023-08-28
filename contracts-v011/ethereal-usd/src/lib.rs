use scrypto::prelude::*;

// let component: Global<AnyComponent> = Global(ObjectStub::new(ObjectStubHandle::Global(GlobalAddress::from(component_address))));
// let return_value = component.call_raw::<ZygomebFancyReturnType>("method_name", scrypto_args!(param1));

#[derive(NonFungibleData, ScryptoSbor)]
pub struct Flash {
  pub size: Decimal,
  pub isloan: Option<bool> // None -- mint, Some(true) -- XRD, Some(true) -- EXRD 
}

// problem: the LP Decimals could run out of Decimal space
#[derive(NonFungibleData, ScryptoSbor)]
pub struct Ecdp {
  #[mutable]
  pub assets_lp: Decimal,
  #[mutable]
  pub liabilities_lp: Decimal
  // TODO liquidation hook
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
      xrdexrd => PUBLIC;
      flash_loan_start => PUBLIC;
      flash_loan_end => PUBLIC;
      flash_mint_start => PUBLIC;
      flash_mint_end => PUBLIC;
      liquidate => PUBLIC;
      tcr => PUBLIC;
      tcr_au_lu => PUBLIC;
      asset_lp_usd => PUBLIC;
      liability_lp_usd => PUBLIC;
      open_ecdp => PUBLIC;
      ecdp_burn => PUBLIC;
      ecdp_mint => PUBLIC;
      ecdp_collateralize => PUBLIC;
      ecdp_uncollateralize => PUBLIC;
      inject_assets => PUBLIC;
      set_oracle => PUBLIC;
      get_oracle => PUBLIC;
      guarded_get_oracle => PUBLIC;
      guarded_get_rescaled_oracle => PUBLIC;
      mock_mint => PUBLIC;
    }
  }

  struct Usd {
    alpha_addr: ComponentAddress,

    power_usd: Vault,

    ecdp_resource: ResourceAddress,

    // lp totals
    assets_lp_total: Decimal,
    liabilities_lp_total: Decimal,

    // eusd
    liabilities_total: Decimal,
    eusd_resource: ResourceAddress,
    
    // xrd + exrd 
    exrd_vault: Vault,
    xrd_vault: Vault,

    assets_index: Decimal,
    liabilities_index: Decimal,

    xrdexrd: Decimal, 
    last_up_xrdexrd: Epoch,

    ep: Decimal,
    mcr: Decimal,
    bp: Decimal,
    ip: Decimal,

    // %-expressed maximum price depeg on open market
    lower_bound: Decimal,
    upper_bound: Decimal,

    // flashing variables, allow only one active flashing in a tx
    // this includes any MP invocation, which means flash self sale is impossible
    flash_resource: ResourceAddress,
    fl_active: bool, 
    fm_active: bool,
    flash_fee: Decimal,

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

      // TODO: metadata
      let ecdp_resource = ResourceBuilder::new_ruid_non_fungible::<Ecdp>(OwnerRole::None)
        .metadata(metadata!(
          roles {
            metadata_setter => rule!(require(power_usd.resource_address()));
            metadata_setter_updater => rule!(deny_all);
            metadata_locker => rule!(deny_all);
            metadata_locker_updater => rule!(deny_all);
          },
          init {
            "name" => "Ethereal ECDP Ownership Badge".to_owned(), updatable;
            "symbol" => "ECDP", updatable;
          }
        ))
        .mint_roles(mint_roles!(
          minter => rule!(require(power_usd.resource_address()));
          minter_updater => rule!(deny_all);
        ))
        // burns aren't utilized so just keeping it here for the uhh ability
        .burn_roles(burn_roles!(
          burner => rule!(require(power_usd.resource_address()));
          burner_updater => rule!(deny_all);
        ))
        .non_fungible_data_update_roles(non_fungible_data_update_roles!(
          non_fungible_data_updater => rule!(require(power_usd.resource_address()));
          non_fungible_data_updater_updater => rule!(deny_all);
        ))
        .create_with_no_initial_supply()
        .address();

      let a1 = Self {
        alpha_addr,

        power_usd: Vault::with_bucket(power_usd),

        ecdp_resource,

        assets_lp_total: dec!(0),
        liabilities_lp_total: dec!(0),

        liabilities_total: dec!(0),
        eusd_resource,
        
        exrd_vault: Vault::new(exrd_resource),
        xrd_vault: Vault::new(XRD),

        assets_index: dec!(1),
        liabilities_index: dec!(1),

        xrdexrd: dec!(1), // TODO call
        last_up_xrdexrd: Runtime::current_epoch(),

        // TODO placeholders
        ep: dec!("1.1"),
        mcr: dec!("1.3"),
        bp: dec!("1.4"),
        ip: dec!("1.6"),

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

    pub fn tcr(&mut self) -> Decimal {
      // TODO call validator
      if let Some(usdxrd) = self.guarded_get_oracle() {
        let usd_xrd = self.xrd_vault.amount() * (dec!(1) / usdxrd); 
        let usd_exrd = self.exrd_vault.amount() * (dec!(1) / (usdxrd * self.xrdexrd(dec!(1))));
        return (usd_xrd + usd_exrd) / self.liabilities_total;
      } 
      panic!("OUTDATED ORACLE");
    }

    // returns tcr + assets in usd + liabilities in usd
    // purely a compt opt
    pub fn tcr_au_lu(&mut self) -> (Decimal, Decimal, Decimal) {
      if let Some(usdxrd) = self.guarded_get_oracle() {
        let usd_xrd = self.xrd_vault.amount() * (dec!(1) / usdxrd); 
        let usd_exrd = self.exrd_vault.amount() * (dec!(1) / (usdxrd * self.xrdexrd(dec!(1))));

        let au = usd_xrd + usd_exrd;
        return (au / self.liabilities_total, au, self.liabilities_total)
      } 
      panic!("OUTDATED ORACLE");
    }

    // conversion of asset_lp units 
    // returns the value of a 1 asset_lp in EUSD
    // i.e. EUSD/asset_lp
    pub fn asset_lp_usd(&mut self) -> Decimal {
      if let Some(usdxrd) = self.guarded_get_oracle() {
        let usd_xrd = self.xrd_vault.amount() * (dec!(1) / usdxrd); 
        let usd_exrd = self.exrd_vault.amount() * (dec!(1) / (usdxrd * self.xrdexrd(dec!(1))));
        return self.assets_index * (usd_xrd + usd_exrd) / self.assets_lp_total;
      }
      panic!("OUTDATED ORACLE");
    }

    // conversion of liability_lp units 
    pub fn liability_lp_usd(&self) -> Decimal {
      return self.liabilities_index * self.liabilities_total / self.liabilities_lp_total;
    }

    // creates an empty ecdp
    pub fn open_ecdp(&mut self) -> Bucket {
      Self::authorize(&mut self.power_usd, || 
        ResourceManager::from(self.ecdp_resource)
          .mint_ruid_non_fungible(
            Ecdp { assets_lp: dec!(0), liabilities_lp: dec!(0) }
          )
      )
    }

    // TODO if stopped
    // if can't mint, panics
    pub fn ecdp_mint(&mut self, lia_lp: Decimal, p: Proof) -> Bucket {
      assert!(lia_lp > dec!(0), 
        "negative mint number");

      let rm = ResourceManager::from(self.ecdp_resource);
      let nft: NonFungible<Ecdp> = p
        .check(self.ecdp_resource)
        .as_non_fungible()
        .non_fungible();
      let id = nft.local_id();
      let data = nft.data();

      let new_liabilities_lp = data.liabilities_lp + lia_lp;

      // if first mint, lia_lp = 1
      let cr = 
        self.asset_lp_usd()*data.assets_lp 
        / ( new_liabilities_lp * self.liabilities_index );
      
      assert!( cr >= self.mcr, 
        "cannot mint under mcr");

      let minted = lia_lp * self.liabilities_index;

      self.liabilities_total += minted;
      self.liabilities_lp_total += lia_lp;
      Self::authorize(&mut self.power_usd, || {
        rm.update_non_fungible_data(&id, "liabilities_lp", 
          lia_lp
        );
        ResourceManager::from(self.eusd_resource).mint(minted)
      })
    }

    // if burns too much, panics
    pub fn ecdp_burn(&mut self, input: Bucket, p: Proof) {
      assert!(!input.is_empty(), 
        "empty input");

      let rm = ResourceManager::from(self.ecdp_resource);
      let nft: NonFungible<Ecdp> = p
        .check(self.ecdp_resource)
        .as_non_fungible()
        .non_fungible();
      let id = nft.local_id();
      let data = nft.data();

      let burn_amount = input.amount();

      let new_liabilities_lp = data.liabilities_lp - burn_amount / self.liabilities_index;
      
      // note: I can imagine a version of the system in which the 
      // negative liabilities make sense
      assert!( new_liabilities_lp >= dec!(0), 
        "negative liabilities");
      
      self.liabilities_total -= burn_amount;
      self.liabilities_lp_total -= burn_amount / self.liabilities_index;
      Self::authorize(&mut self.power_usd, || {
        rm.update_non_fungible_data(&id, "liabilities_lp", 
          new_liabilities_lp
        );
        ResourceManager::from(self.eusd_resource).burn(input);
      });
    }

    // absolutely no panics, ever
    pub fn ecdp_collateralize(&mut self, input: Bucket, p: Proof) {
      assert!(!input.is_empty(), 
        "empty input");

      let rm = ResourceManager::from(self.ecdp_resource);
      let nft: NonFungible<Ecdp> = p
        .check(self.ecdp_resource)
        .as_non_fungible()
        .non_fungible();
      let id = nft.local_id();
      let data = nft.data();

      let added_assets_lp = 
        if input.resource_address() == self.exrd_vault.resource_address()
        { let out = self.xrdexrd(input.amount()) / self.assets_index;
          self.exrd_vault.put(input);
          out
        } else {
          let out = input.amount() / self.assets_index;
          self.xrd_vault.put(input);
          out
        };
      
      self.assets_lp_total += added_assets_lp;
      Self::authorize(&mut self.power_usd, || {
        rm.update_non_fungible_data(&id, "assets_lp", 
          data.assets_lp + added_assets_lp
        );
      });
    }

    // returns EXRD first, and if that runs out, XRD second
    pub fn ecdp_uncollateralize(&mut self, ass_lp: Decimal, p: Proof) 
      -> (Bucket, Option<Bucket>) {
      assert!(ass_lp != dec!(0), 
        "empty input");

      let rm = ResourceManager::from(self.ecdp_resource);
      let nft: NonFungible<Ecdp> = p
        .check(self.ecdp_resource)
        .as_non_fungible()
        .non_fungible();
      let id = nft.local_id();
      let data = nft.data();

      let new_assets_lp = data.assets_lp - ass_lp;

      assert!( new_assets_lp >= dec!(0),
        "negative assets");

      let mut ret_xrd = None;
      let ret_exrd = {
        let refund_xrd = self.assets_index * ass_lp;

        if self.xrdexrd(self.exrd_vault.amount()) < refund_xrd {
          // if the eexrd vault alone cannot pay out enough
          let paidout = self.xrdexrd(self.exrd_vault.amount());
          ret_xrd = Some(self.xrd_vault.take(refund_xrd - paidout));
          self.exrd_vault.take_all()
        } else {
          self.exrd_vault.take(refund_xrd / self.xrdexrd)
        }
      };
      
      self.assets_lp_total -= ass_lp;
      Self::authorize(&mut self.power_usd, || {
        rm.update_non_fungible_data(&id, "assets_lp", 
          new_assets_lp
        );
      });

      (ret_exrd, ret_xrd)
    }

    // takes an id of the ECDP to liquidate
    // if liquidated, returns 1% of the total assets as a liquidator tip
    // i.e. makes them push the button even if the ecdp is bad debt
    // the rest is subtracted from top and bottom 1:1
    // whatever assets are left, remain active and the game continues
    pub fn liquidate(&mut self, 
      liquidated_id: NonFungibleLocalId, 
      liquidator_id: NonFungibleLocalId) {

      assert!( !self.fl_active && !self.fm_active,
        "can't liquidate during flash transactions");
      
      let rm = ResourceManager::from(self.ecdp_resource);
      let data_ted: Ecdp = rm.get_non_fungible_data(&liquidated_id);

      let ass_usd = self.asset_lp_usd();
      let lia_usd = self.liability_lp_usd();
      let liq_cr = (data_ted.assets_lp * ass_usd) 
              / (data_ted.liabilities_lp * lia_usd);

      // CR must fall under MCR or value of assets under 30 bucks
      if liq_cr >= self.mcr && ( ass_usd >= dec!(30) ) {
        // no liquidation
        return 
      }

      // TODO liquidation hook

      // after tor cut --- todo, tor cut a parameter?
      let assets_lp_total = data_ted.assets_lp * dec!("0.99");
      let tor_cut = data_ted.assets_lp - assets_lp_total;
      let assets_lp_usd_total = assets_lp_total * ass_usd;

      let mut ted_remaining_usd = assets_lp_usd_total - data_ted.liabilities_lp * lia_usd;

      // if bad debt, wipe out
      if ted_remaining_usd <= dec!(0) {
        ted_remaining_usd = dec!(0);
      }

      let ted_remaining_assets = ted_remaining_usd * (dec!(1) / ass_usd);

      let a_prior = self.assets_lp_total;
      self.assets_lp_total -= assets_lp_total - ted_remaining_assets;
      let l_prior = self.liabilities_lp_total;
      self.liabilities_lp_total -= data_ted.liabilities_lp;

      self.assets_index *= a_prior / self.assets_lp_total;
      self.liabilities_index *= l_prior / self.liabilities_lp_total;

      let data_tor: Ecdp = rm.get_non_fungible_data(&liquidator_id);
      Self::authorize(&mut self.power_usd, || {
        rm.update_non_fungible_data(&liquidated_id, "assets_lp", 
          ted_remaining_assets
        );
        rm.update_non_fungible_data(&liquidated_id, "liabilities_lp",
          dec!(0)
        );
        
        rm.update_non_fungible_data(&liquidator_id, "assets_lp", 
          data_tor.assets_lp + tor_cut
        );
      });
    }

    // TODO: check that this is called on every asset_lp interaction
    // how much XRD is each EXRD worth, i.e. xrd/exrd 
    // system assumes no time value on unstake 
    // input as size in EXRD, inp 1 ~> returns >= 1
    // additionally it corrects the asset index by bumping it with stake rewards from exrd
    pub fn xrdexrd(&mut self, size: Decimal) -> Decimal {
      // TODO call validator
      // https://github.com/radixdlt/radixdlt-scrypto/blob/main/
      // radix-engine/src/blueprints/consensus_manager/validator.rs#L1037
      // ^ reeeeeee
      // if they don't fix -> add to oracle

      let current = Runtime::current_epoch();
      if self.last_up_xrdexrd < current {
        let totalxrdpre = self.xrd_vault.amount() + self.xrdexrd*self.exrd_vault.amount();
        self.xrdexrd = dec!(1); // todo update 
        let totalxrdpost = self.xrd_vault.amount() + self.xrdexrd*self.exrd_vault.amount();

        self.assets_index *= totalxrdpost/totalxrdpre;
        self.last_up_xrdexrd = current;
      }
      size * self.xrdexrd
    }

    // Flash Mint / Loan parts

    // re: flash mint/loans
    // they cannot trigger MP as that panics
    // so any 'forced liquidation' has to go thru an external market on EXRD/EUSD

    // if res -- exrd, otherwise xrd
    // repayment in either
    pub fn flash_loan_start(&mut self, size: Decimal, res: bool) -> (Bucket, Bucket) {
      assert!(size <= if res { self.exrd_vault.amount() } else { self.xrd_vault.amount() },
        "our size is not size enough"
      );
      assert!(!self.fl_active, 
        "twice flash loaned");

      self.fl_active = true;

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
      assert!(self.fl_active, 
        "twice flash loaned");

      let data: Flash = flash.as_non_fungible().non_fungible().data();

      let pre = data.size / self.flash_fee; 
      let rest = self.xrdexrd(self.exrd_vault.amount()) + self.xrd_vault.amount();
      
      match data.isloan {
        Some(true) => {
          if input.resource_address() == self.exrd_vault.resource_address() {
            assert!(input.amount() >= data.size,
              "insufficient size");
            
            self.assets_index *= 
              (rest + self.xrdexrd * input.amount()) 
              / (rest + self.xrdexrd * pre);

            self.exrd_vault.put( input );
          } else {
            assert!(input.amount() >= self.xrdexrd(data.size),
              "insufficient size");
            
            self.assets_index *= 
              (rest + input.amount()) 
              / (rest + self.xrdexrd * pre);

            self.xrd_vault.put( input );
          }
        }
        Some(false) => {
          if input.resource_address() == self.exrd_vault.resource_address() {   
            assert!(self.xrdexrd(input.amount()) >= data.size,
              "insufficient size");

            self.assets_index *= 
              (rest + self.xrdexrd * input.amount()) 
              / (rest + pre);

            self.exrd_vault.put(input);
          } else {
            assert!(input.amount() >= data.size,
              "insufficient size");

            self.assets_index *= 
              (rest + input.amount()) 
              / (rest + pre);

            self.xrd_vault.put(input);
          }
        }
        None => panic!("wrong resource type")
      };

      self.fl_active = false;
      Self::authorize(&mut self.power_usd, || {
        ResourceManager::from(self.flash_resource).burn(flash)
      })
    }

    // TODO: impose limit on the size?
    // what could go wrong? lmao
    pub fn flash_mint_start(&mut self, size: Decimal) -> (Bucket, Bucket) {
      // the liablitity # doesn't change until repayment
      assert!(!self.fm_active, 
        "twice flash minted");

      Self::authorize(&mut self.power_usd, || {
        let flash = ResourceManager::from(self.flash_resource)
          .mint_ruid_non_fungible(
            Flash {
              size: size*self.flash_fee,
              isloan: None
          });

        self.fm_active = true;

        (ResourceManager::from(self.eusd_resource).mint(size), flash)
      })
    }

    pub fn flash_mint_end(&mut self, input: Bucket, flash: Bucket) {
      assert!(self.fm_active, 
        "twice flash minted");
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

      self.fm_active = false;
      let pre = self.liabilities_total;
      self.liabilities_total -= input.amount() * (dec!(1) - self.flash_fee);
      self.liabilities_index *= self.liabilities_total / pre;

      Self::authorize(&mut self.power_usd, || {
        ResourceManager::from(self.eusd_resource).burn(input);
        ResourceManager::from(self.flash_resource).burn(flash);
      });
    }

    // Automatic Arbitrage / Mandatory Pegging parts

    // check if aa is necessary
    // contains all the mandatory pegging logic
    // for v2, to maybe AMO-ize that
    // spot is EUSD/EXRD
    pub fn aa_poke(&mut self, spot: Decimal) -> Option<(Decimal, Decimal, bool)> {
      info!("aa_poke IN"); 
      // todo panic if flashed
      // ^ is it even needed? shouldn't be a problem really

      // EUSD/EXRD
      let usdexrd = self.guarded_get_rescaled_oracle().expect("OUTDATED ORACLE");

      if spot > usdexrd * self.upper_bound {
        Some((usdexrd * self.upper_bound, usdexrd, true))
      } else if spot < usdexrd * self.lower_bound {
        Some((usdexrd * self.lower_bound, usdexrd, false))
      } else { 
        None
      }
    }

    // execute aa
    pub fn aa_woke(&mut self, size: Decimal, direction: bool) 
      -> Option<(Bucket, (Decimal, Decimal, Decimal)) > {
      info!("aa_woke IN"); 
      let (tcr, au, lu) = self.tcr_au_lu();

      if direction {
        // above backstop, can do MPup
        if tcr > self.bp {
          // derived from 
          // bp <= a / (l + mint)
          let max_mint = (au - lu * self.bp) / self.bp;
          let mint = if max_mint > size { size } else { max_mint };

          assert!( mint < au && mint < lu,
            "incoherence" );

          Self::authorize(&mut self.power_usd, || {
            self.liabilities_total += mint;
            Some((ResourceManager::from(self.eusd_resource).mint(mint), (mint, au, lu)))
          })
        } else {
          None
        }
      } else {
        // above emergency, can do MPdown
        if tcr > self.ep {
          Self::authorize(&mut self.power_usd, || {            
            if self.exrd_vault.amount() <= size {
              panic!("todo -- NEEDS VALIDATOR API");
              // TODO: pull liq from XRD and change into EXRD 
              // (and if that's not enough, protocol is broke lol)
            } else {
              Some((self.exrd_vault.take(size), (size, au, lu)))
            }
          })
        } else {
          None
        }
      }
    }
    
    // get aa profit, in LP
    // input is EUXLP -> Treasury to be changed into TLP
    // remainder is of type dep on direction -- incoherence panics
    pub fn aa_choke(&mut self, ret: Bucket, profit: Bucket, direction: bool,
      prior: (Decimal, Decimal, Decimal)) {
      info!("aa_choke IN"); 

      let (size, au, lu) = prior;
      let tcr = au/lu;

      let alpha: Global<AnyComponent> = self.alpha_addr.into();
      if direction {
        let ret_usd = ret.amount() / self.guarded_get_rescaled_oracle().unwrap();

        // Down state or Up state?
        if tcr > self.ip {
          // top right -> owe less, own less
          self.assets_index *= (au - size) / au;
          self.liabilities_index *= (lu - ret_usd) / lu;
        } else {
          // top left -> owe more, own more
          self.liabilities_index *= (lu + ret_usd) / lu;
          self.assets_index *= (au + size) / au;
        }

        self.exrd_vault.put(ret);

        // todo: authorize question on alpha
        alpha.call_raw::<()>("aa_rope", scrypto_args!(profit));
      } else {
        // todo double check

        let size_usd = size / self.guarded_get_rescaled_oracle().unwrap();

        let c = 
          (self.xrd_vault.amount() + self.xrdexrd*(self.exrd_vault.amount() + size))
          / self.assets_lp_total;

        // ensures that assets index doesn't go above total assets
        if tcr > self.ip && self.assets_index * ((au + size_usd) / au) < c {
          // bottom right -> owe more, own more
          self.assets_index *= (au + ret.amount()) / au;
          self.liabilities_index *= (lu + size_usd) / lu;
        } else {
          // bottom left -> owe less, own less
          self.assets_index *= (au - size_usd) / au;
          self.liabilities_index *= (lu - ret.amount()) / lu;
        }

        self.liabilities_total -= ret.amount();
        Self::authorize(&mut self.power_usd, || {
          ResourceManager::from(self.eusd_resource).burn(ret)
        });

        // todo: authorize question on alpha
        alpha.call_raw::<()>("aa_rope", scrypto_args!(profit));
      }
    }

    // internal 

    // returns USD/EXRD
    pub fn guarded_get_rescaled_oracle(&mut self) -> Option<Decimal> {
      if let Some(usdxrd) = self.guarded_get_oracle() {
        // USD/EXRD = USD/XRD * XRD/EXRD 
        Some(usdxrd * self.xrdexrd(dec!(1)))
      } else {
        None
      }
    }

    // if feed is outdated, stop the system / withdrawal only mode
    pub fn guarded_get_oracle(&self) -> Option<Decimal> {
      let (usdxrd, last_update) = self.get_oracle();

      // if oracle inactive for 5 minutes, shit the bed
      if last_update.add_minutes(5i64).expect("incoherence").compare(
          Clock::current_time_rounded_to_minutes(),
          TimeComparisonOperator::Lte
       ) {
        None
      } else {
        Some(usdxrd)
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

    // USD/XRD and last time it was updated
    pub fn get_oracle(&self) -> (Decimal,Instant) {
      (self.mock_oracle, Clock::current_time_rounded_to_minutes())
    }
  }
}