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

// events

#[derive(ScryptoSbor, ScryptoEvent)]
struct EcdpAssetsEvent {
  ecdp: NonFungibleLocalId,
  diff: Decimal, // negative = outflow
  new: Decimal
}

#[derive(ScryptoSbor, ScryptoEvent)]
struct EcdpLiabilitiesEvent {
  ecdp: NonFungibleLocalId,
  diff: Decimal, // negative = outflow
  new: Decimal
}

// sizes shown in the above events which are also emited
#[derive(ScryptoSbor, ScryptoEvent)]
struct EcdpLiquidatedEvent {
  ecdp: NonFungibleLocalId
}

#[derive(ScryptoSbor, ScryptoEvent)]
struct NewEcdpEvent {
  ecdp: NonFungibleLocalId
}

#[derive(ScryptoSbor, ScryptoEvent)]
struct FlashEvent {
  size: Decimal,
  isloan: Option<bool> 
}

#[derive(ScryptoSbor, ScryptoEvent)]
struct AAEvent {
  direction: bool,
  size: Decimal,
  profit: Decimal
}

#[derive(ScryptoSbor, ScryptoEvent)]
struct OracleEvent {
  old: Decimal,
  new: Decimal
}

// code

#[blueprint]
mod usd {
  enable_method_auth! {
    roles {
      azero => updatable_by: [];
      dex => updatable_by: [azero]; // temporary measure, TODO: alpha + delpoy braider
    },
    methods {
      to_nothing => restrict_to: [azero]; //todo alpha's power zero
      start_stop => restrict_to: [azero];
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
      tcr_au_lu => PUBLIC;
      asset_lp_usd => PUBLIC;
      asset_lp_xrd => PUBLIC;
      liability_lp_usd => PUBLIC;
      open_ecdp => PUBLIC;
      ecdp_burn => PUBLIC;
      ecdp_mint => PUBLIC;
      ecdp_collateralize => PUBLIC;
      ecdp_uncollateralize => PUBLIC;
      set_oracle => PUBLIC;
      get_oracle => PUBLIC;
      guarded_get_oracle => PUBLIC;
      guarded_get_rescaled_oracle => PUBLIC;
      look_within => PUBLIC;
      get_params => PUBLIC;
      set_params => restrict_to: [azero];
      first_ecdp => restrict_to: [azero];
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
    exrd_validator: ComponentAddress,

    ep: Decimal,
    mcr: Decimal,
    bp: Decimal,

    // %-expressed maximum price depeg on open market
    lower_bound: Decimal,
    upper_bound: Decimal,

    // for safety reasons markets have to be sized appropriately
    // at least for now 
    maximum_minted: Decimal,

    // flashing variables, allow only one active flashing in a tx
    // this includes any MP invocation, which means flash self sale is impossible
    flash_resource: ResourceAddress,
    fl_active: bool, 
    fm_active: bool,
    flash_fee: Decimal,

    // exch rate
    oracle: Decimal,
    oracle_timestamp: Instant,
    oracle1: ResourceAddress,
    oracle2: ResourceAddress,

    stopped: bool // TODO make work
  }

  impl Usd {
    pub fn from_nothing(
      alpha_addr: ComponentAddress, power_azero: ResourceAddress,
      power_eux: ResourceAddress, power_usd: Bucket, exrd_resource: ResourceAddress, 
      exrd_validator: ComponentAddress, 
      lower_bound: Decimal, upper_bound: Decimal, flash_fee: Decimal, bang: ComponentAddress,
      oracle_init: Decimal, oracle1: ResourceAddress, oracle2: ResourceAddress
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
            "icon_url" => 
              Url::of("https://cdn.discordapp.com/attachments/1092987092864335884/1149733059118239774/logos1_1_1.jpeg")
              , updatable;
            "dapp_definitions" =>
              vec!(GlobalAddress::from(bang)), updatable;
            "tags" => vec!["ethereal-dao".to_owned(), "stablecoin".to_owned()], updatable;
            "info_url" => Url::of("https://ethereal.systems"), updatable;
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
            "key_image_url" => 
              Url::of("https://cdn.discordapp.com/attachments/1092987092864335884/1095874817758081145/logos1.jpeg")
              , updatable;
            "dapp_definitions" =>
              vec!(GlobalAddress::from(bang)), updatable;
            "tags" => vec!["ethereal-dao".to_owned(), "ecdp".to_owned(), "loan-positions".to_owned()], updatable;
            "info_url" => Url::of("https://ethereal.systems"), updatable;
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
        exrd_validator,

        // TODO candidate numbers
        ep: dec!("1.2"),
        mcr: dec!("1.5"),
        bp: dec!("1.7"),

        lower_bound,
        upper_bound,

        // TODO candidate number
        maximum_minted: dec!("1000000"),

        flash_resource,
        fl_active: false,
        fm_active: false,
        flash_fee,

        oracle: oracle_init,
        oracle_timestamp: Clock::current_time_rounded_to_minutes(),
        oracle1,
        oracle2,

        stopped: false // TODO: need vote to start
      }
      .instantiate()
      .prepare_to_globalize(OwnerRole::None)
      .roles(
        roles!(
          azero => rule!(require(power_azero));
          dex => rule!(require(power_eux));
        )
      )
      .metadata(
        metadata!(
          roles {
            metadata_setter => rule!(require(power_azero));
            metadata_setter_updater => rule!(deny_all);
            metadata_locker => rule!(deny_all);
            metadata_locker_updater => rule!(deny_all);
          },
          init {
            "dapp_definition" =>
              GlobalAddress::from(bang), updatable;
            "tags" => vec!["ethereal-dao".to_owned(), 
              "usd".to_owned(), "stablecoin".to_owned()], updatable;
          }
        )
      )
      .globalize()
      .address();

      return (a1, eusd_resource)
    }

    pub fn to_nothing(&mut self) -> (Bucket, Bucket, Bucket) {
      (
        self.power_usd.take_all(),
        self.exrd_vault.take_all(),
        self.xrd_vault.take_all()
      )
    }
    
    pub fn look_within(&self) 
      -> (Decimal, Decimal, Decimal, Decimal) {
      (
        self.assets_lp_total,
        self.liabilities_lp_total,
        self.liabilities_total,
        self.oracle
      )
    }

    pub fn start_stop(&mut self, input: bool) {
      self.stopped = input;
    }

    // easy access
    pub fn get_params(&self) -> (Decimal, Decimal, Decimal, Decimal, Decimal, Decimal, Decimal) {
      (
        self.ep,
        self.mcr,
        self.bp,
        self.lower_bound,
        self.upper_bound,
        self.maximum_minted,
        self.flash_fee
      )
    }

    // easy change
    pub fn set_params(&mut self, 
      ep: Decimal, mcr: Decimal, bp: Decimal, lb: Decimal, ub: Decimal, mm: Decimal, ff: Decimal) {
      
      self.ep = ep;
      self.mcr = mcr;
      self.bp = bp;
      self.lower_bound = lb;
      self.upper_bound = ub;
      self.maximum_minted = mm;
      self.flash_fee = ff;
    }

    pub fn tcr(&mut self) -> Decimal {
      if let Some(xrdusd) = self.guarded_get_oracle() {
        let usd_xrd = self.xrd_vault.amount() * xrdusd; 
        let usd_exrd = self.exrd_vault.amount() * xrdusd * self.exrdxrd();
        return (usd_xrd + usd_exrd) / self.liabilities_total;
      } 
      panic!("OUTDATED ORACLE");
    }

    // returns tcr + assets in usd + liabilities in usd
    // purely a compt opt
    pub fn tcr_au_lu(&mut self) -> (Decimal, Decimal, Decimal) {
      if let Some(xrdusd) = self.guarded_get_oracle() {
        let usd_xrd = self.xrd_vault.amount() * xrdusd; 
        let usd_exrd = self.exrd_vault.amount() * xrdusd * self.exrdxrd();

        let au = usd_xrd + usd_exrd;
        return (au / self.liabilities_total, au, self.liabilities_total)
      } 
      panic!("OUTDATED ORACLE");
    }

    // how much XRD is each EXRD worth, i.e. exrd/xrd 
    // system assumes no time value on unstake 
    // input as size in EXRD, inp 1 ~> returns >= 1
    // additionally it corrects the asset index by bumping it with stake rewards from exrd
    pub fn exrdxrd(&self) -> Decimal {
      // fuck type safety
      let valid: Global<AnyComponent> = self.exrd_validator.into();

      valid.call_raw("get_redemption_value", scrypto_args!(dec!(1)))
    }

    // conversion of asset_lp units 
    // returns the value of a 1 asset_lp in EUSD
    // i.e. asset_lp/EUSD
    pub fn asset_lp_usd(&mut self) -> Decimal {
      if let Some(xrdusd) = self.guarded_get_oracle() {
        if self.assets_lp_total == dec!(0) {
          return xrdusd
        }
        let usd_xrd = self.xrd_vault.amount() * xrdusd; 
        let usd_exrd = self.exrd_vault.amount() * xrdusd * self.exrdxrd();
        return (usd_xrd + usd_exrd) / self.assets_lp_total;
      }
      panic!("OUTDATED ORACLE");
    }

    // asset_lp/XRD
    // how much xrd is 1 asset_lp worth
    pub fn asset_lp_xrd(&mut self) -> Decimal {
      if self.assets_lp_total == dec!(0) {
        return dec!(1)
      }

      ( self.xrd_vault.amount() + self.exrdxrd()*self.exrd_vault.amount() ) 
        / self.assets_lp_total
    }

    // conversion of liability_lp units 
    // lia_lp / EUSD
    pub fn liability_lp_usd(&self) -> Decimal {
      if self.liabilities_lp_total == dec!(0) {
        return dec!(1)
      }
      return self.liabilities_total / self.liabilities_lp_total;
    }

    // can technically create it underwater but that doesn't matter
    // intended use is for treasury to create it at like 5x overcollat and not manage it at all
    // ASSUMES EXRD INPUT, MINTS 777 EUSD, NO CR CHECKS
    pub fn first_ecdp(&mut self, input: Bucket) -> (Bucket, Bucket) {
      assert!( self.liabilities_lp_total == dec!(0) && self.assets_lp_total == dec!(0),
        "not the first ecdp" );

      let ecdp = Self::authorize(&mut self.power_usd, || 
        ResourceManager::from(self.ecdp_resource)
          .mint_ruid_non_fungible(
            Ecdp { assets_lp: dec!(0), liabilities_lp: dec!(0) }
          )
      );

      let nft: NonFungible<Ecdp> = ecdp.as_non_fungible().non_fungible();
      let id = nft.local_id();

      let assets_lp = self.exrdxrd()*input.amount();
      let liabilities_lp = dec!("777");

      self.assets_lp_total += assets_lp;
      self.liabilities_lp_total += liabilities_lp;
      self.liabilities_total = liabilities_lp;

      Runtime::emit_event(
        NewEcdpEvent { ecdp: id.clone() });
      Runtime::emit_event(
        EcdpAssetsEvent { ecdp: id.clone(), diff: assets_lp, new: assets_lp });
      Runtime::emit_event(
        EcdpLiabilitiesEvent { ecdp: id.clone(), diff: liabilities_lp, new: liabilities_lp });

      let eusd = Self::authorize(&mut self.power_usd, || {
        let rm = ResourceManager::from(self.ecdp_resource);
        rm.update_non_fungible_data(&id, "assets_lp", 
          assets_lp
        );
        rm.update_non_fungible_data(&id, "liabilities_lp", 
          liabilities_lp
        );
        ResourceManager::from(self.eusd_resource).mint(dec!("777"))
      });

      (ecdp, eusd)
    }

    // creates an empty ecdp
    pub fn open_ecdp(&mut self, fee: Bucket) -> Bucket {
      assert!( self.liabilities_lp_total != dec!(0) && self.assets_lp_total != dec!(0),
        "need a first ecdp first" );

      assert!( fee.amount() < dec!("100"), 
        "fee too small" );

      self.xrd_vault.put(fee);
      
      Self::authorize(&mut self.power_usd, || {
        let out = ResourceManager::from(self.ecdp_resource)
          .mint_ruid_non_fungible(
            Ecdp { assets_lp: dec!(0), liabilities_lp: dec!(0) }
          );
        Runtime::emit_event(
          NewEcdpEvent { ecdp: out.as_non_fungible().non_fungible_local_id() });

        out
      })
    }

    // if can't mint, panics
    pub fn ecdp_mint(&mut self, lia_lp: Decimal, p: Proof) -> Bucket {
      assert!( !self.stopped && !self.power_usd.is_empty(),
        "USD stopped or empty"); 
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

      let lp_usd = self.liability_lp_usd();

      let cr = 
        self.asset_lp_usd()*data.assets_lp 
        / ( new_liabilities_lp * lp_usd );
      assert!( cr >= self.mcr, 
        "cannot mint under mcr");
      assert!( self.liabilities_total + new_liabilities_lp * lp_usd > self.maximum_minted, 
        "exceeded maximum minted");

      Runtime::emit_event(
        EcdpLiabilitiesEvent { ecdp: id.clone(), diff: lia_lp, new: new_liabilities_lp });

      let minted = lia_lp * lp_usd;

      self.liabilities_total += minted;
      self.liabilities_lp_total += lia_lp;
      Self::authorize(&mut self.power_usd, || {
        rm.update_non_fungible_data(&id, "liabilities_lp", 
          new_liabilities_lp
        );
        ResourceManager::from(self.eusd_resource).mint(minted)
      })
    }

    // if burns too much, panics
    pub fn ecdp_burn(&mut self, input: Bucket, p: Proof) {
      assert!( !self.stopped && !self.power_usd.is_empty(),
        "USD stopped or empty"); 
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
      let lp_usd = self.liability_lp_usd();

      // prop_val = lp_usd * lp ~> lp = prop_val / lp_usd 
      let new_liabilities_lp = data.liabilities_lp - burn_amount / lp_usd;
      
      // note: I can imagine a version of the system in which the 
      // negative liabilities make sense
      assert!( new_liabilities_lp >= dec!(0), 
        "negative liabilities");

      Runtime::emit_event(
        EcdpLiabilitiesEvent { 
          ecdp: id.clone(), 
          diff: dec!("-1") * burn_amount / lp_usd, 
          new: new_liabilities_lp });
      
      self.liabilities_total -= burn_amount;
      self.liabilities_lp_total -= burn_amount / lp_usd;
      Self::authorize(&mut self.power_usd, || {
        rm.update_non_fungible_data(&id, "liabilities_lp", 
          new_liabilities_lp
        );
        ResourceManager::from(self.eusd_resource).burn(input);
      });
    }

    // absolutely no panics, ever
    pub fn ecdp_collateralize(&mut self, input: Bucket, p: Proof) {
      assert!( !self.stopped && !self.power_usd.is_empty(),
        "USD stopped or empty"); 
      assert!(!input.is_empty(), 
        "empty input");

      let rm = ResourceManager::from(self.ecdp_resource);
      let nft: NonFungible<Ecdp> = p
        .check(self.ecdp_resource)
        .as_non_fungible()
        .non_fungible();
      let id = nft.local_id();
      let data = nft.data();

      let size = input.amount();

      let added_assets_lp = 
        if input.resource_address() == self.exrd_vault.resource_address()
        { let new = self.exrdxrd()*size / self.asset_lp_xrd();
          self.exrd_vault.put(input);
          new
        } else {
          let new = size / self.asset_lp_xrd();
          self.xrd_vault.put(input);
          new
        };
      
      Runtime::emit_event(
        EcdpAssetsEvent { 
          ecdp: id.clone(), 
          diff: added_assets_lp, 
          new: data.assets_lp + added_assets_lp });
      
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
      assert!( !self.stopped && !self.power_usd.is_empty(),
        "USD stopped or empty"); 
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

      let cr = 
        new_assets_lp * self.asset_lp_usd()
        / ( data.liabilities_lp * self.liability_lp_usd() );
      assert!( cr >= self.mcr, 
        "cannot mint under mcr");

      let lp_xrd = self.asset_lp_xrd();

      let mut ret_xrd = None;
      let ret_exrd = {
        let refund_xrd = ass_lp * lp_xrd;
        let exrdxrd =  self.exrdxrd();

        if exrdxrd*self.exrd_vault.amount() < refund_xrd {
          // if the eexrd vault alone cannot pay out enough
          let paidout = exrdxrd*self.exrd_vault.amount();
          ret_xrd = Some(self.xrd_vault.take(refund_xrd - paidout));
          self.exrd_vault.take_all()
        } else {
          self.exrd_vault.take(refund_xrd / exrdxrd)
        }
      };

      Runtime::emit_event(
        EcdpAssetsEvent { ecdp: id.clone(), diff: dec!("-1")*ass_lp, new: new_assets_lp });
      
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
      assert!( !self.stopped && !self.power_usd.is_empty(),
        "USD stopped or empty"); 
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

      self.assets_lp_total -= assets_lp_total - ted_remaining_assets;
      self.liabilities_lp_total -= data_ted.liabilities_lp;

      let data_tor: Ecdp = rm.get_non_fungible_data(&liquidator_id);
      Runtime::emit_event(
        EcdpLiquidatedEvent { 
          ecdp: liquidated_id.clone() });
      Runtime::emit_event(
        EcdpAssetsEvent { 
          ecdp: liquidated_id.clone(), 
          diff: ted_remaining_assets - data_ted.assets_lp, 
          new: ted_remaining_assets });
      Runtime::emit_event(
        EcdpLiabilitiesEvent { 
          ecdp: liquidated_id.clone(), 
          diff: dec!("-1") * data_ted.liabilities_lp, 
          new: dec!("0") });
      Runtime::emit_event(
        EcdpAssetsEvent { 
          ecdp: liquidator_id.clone(), 
          diff: tor_cut, 
          new: data_tor.assets_lp + tor_cut });
      
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

    // Flash Mint / Loan parts

    // re: flash mint/loans
    // they cannot trigger MP as that panics
    // so any 'forced liquidation' has to go thru an external market on EXRD/EUSD

    // if res -- exrd, otherwise xrd
    // repayment in either
    pub fn flash_loan_start(&mut self, size: Decimal, res: bool) -> (Bucket, Bucket) {
      assert!( !self.stopped && !self.power_usd.is_empty(),
        "USD stopped or empty"); 
      assert!(size <= if res { self.exrd_vault.amount() } else { self.xrd_vault.amount() },
        "our size is not size enough"
      );
      assert!(!self.fl_active, 
        "twice flash loaned");


      // TODO : in unindexified version when flash loaned the system
      //        needs to work correctly 
      //        despite "technically" possibly being underwater
      //
      //        take 1: block MP and liq when in any way flashed 

      self.fl_active = true;

      Runtime::emit_event(
        FlashEvent { 
          size: size, 
          isloan: Some(res) });

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

      let exrdxrd = self.exrdxrd();
      
      match data.isloan {
        Some(true) => {
          if input.resource_address() == self.exrd_vault.resource_address() {
            assert!(input.amount() >= data.size,
              "insufficient size");

            self.exrd_vault.put( input );
          } else {
            assert!(input.amount() >= exrdxrd*data.size,
              "insufficient size");

            self.xrd_vault.put( input );
          }
        }
        Some(false) => {
          if input.resource_address() == self.exrd_vault.resource_address() {   
            assert!(exrdxrd*input.amount() >= data.size,
              "insufficient size");

            self.exrd_vault.put(input);
          } else {
            assert!(input.amount() >= data.size,
              "insufficient size");

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
      assert!( !self.stopped && !self.power_usd.is_empty(),
        "USD stopped or empty"); 
      // the liablitity # doesn't change until repayment
      assert!(!self.fm_active, 
        "twice flash minted");

      Runtime::emit_event(
        FlashEvent { 
          size: size, 
          isloan: None });

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
      self.liabilities_total -= input.amount() * (dec!(1) - self.flash_fee);

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

      assert!( !self.fl_active && !self.fm_active,
        "can't liquidate during flash transactions");

      // EUSD/EXRD
      let usdexrd = dec!(1) / self.guarded_get_rescaled_oracle().expect("OUTDATED ORACLE");

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
      -> Option<Bucket> {
      info!("aa_woke IN"); 
      let (tcr, au, lu) = self.tcr_au_lu();

      if direction {
        // above backstop, can do MPup
        if tcr > self.bp {
          // derived from 
          // bp <= a / (l + mint)
          let mut max_mint = (au - lu * self.bp) / self.bp;
          // pick smaller out of the CR limit and the static size limit
          max_mint = if max_mint > self.maximum_minted - self.liabilities_total {
              self.maximum_minted - self.liabilities_total 
            } else {
              max_mint
            };
          let mint = if max_mint > size { size } else { max_mint };

          assert!( mint < au && mint < lu,
            "incoherence" );

          Self::authorize(&mut self.power_usd, || {
            self.liabilities_total += mint;
            Some(ResourceManager::from(self.eusd_resource).mint(mint))
          })
        } else {
          None
        }
      } else {
        let exrdxrd = self.exrdxrd();
        // above emergency, can do MPdown
        if tcr > self.ep {
          Self::authorize(&mut self.power_usd, || {            
            if self.exrd_vault.amount() <= size {
              let valid: Global<AnyComponent> = self.exrd_validator.into();
              let diff = size - self.exrd_vault.amount();
              let reqxrd = diff * dec!(1) / exrdxrd;

              if reqxrd >= self.xrd_vault.amount() {
                let newexrd = valid.call_raw(
                  "stake",
                  scrypto_args!(self.xrd_vault.take(reqxrd))
                );
                self.exrd_vault.put(newexrd);

                Some(self.exrd_vault.take(size))
              } else {
                // (protocol is broke lol)
                // returnning None, no good option here
                // TODO: maybe halt the system?
                None
              }
            } else {
              Some(self.exrd_vault.take(size))
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
    pub fn aa_choke(&mut self, ret: Bucket, profit: Bucket, direction: bool) {
      info!("aa_choke IN"); 

      info!("stage 1");
      let alpha: Global<AnyComponent> = self.alpha_addr.into();
      Runtime::emit_event(
        AAEvent { 
          direction,
          size: ret.amount(),
          profit: profit.amount() });
      if direction {
        self.exrd_vault.put(ret);

        // todo: authorize question on alpha
        alpha.call_raw::<()>("aa_rope", scrypto_args!(profit));
      } else {
        // todo double check
        self.liabilities_total -= ret.amount();
        Self::authorize(&mut self.power_usd, || {
          ResourceManager::from(self.eusd_resource).burn(ret)
        });

        // todo: authorize question on alpha
        alpha.call_raw::<()>("aa_rope", scrypto_args!(profit));
      }
    }

    // internal 

    // returns EXRD/USD
    pub fn guarded_get_rescaled_oracle(&mut self) -> Option<Decimal> {
      if let Some(xrdusd) = self.guarded_get_oracle() {
        // EXRD/USD = XRD/USD * EXRD/XRD 
        Some(xrdusd * self.exrdxrd())
      } else {
        None
      }
    }

    // if feed is outdated, stop the system / withdrawal only mode
    pub fn guarded_get_oracle(&self) -> Option<Decimal> {
      let (xrdusd, last_update) = self.get_oracle();

      // if oracle inactive for 5 minutes, shit the bed
      if last_update.add_minutes(5i64).expect("incoherence").compare(
          Clock::current_time_rounded_to_minutes(),
          TimeComparisonOperator::Lte
       ) {
        None
      } else {
        Some(xrdusd)
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

    pub fn set_oracle(&mut self, exch: Decimal, p: Proof) {

      Runtime::emit_event(OracleEvent { old: self.oracle, new: exch });

      if p.resource_address() == self.oracle1 {
        self.oracle = exch;
        return
      }

      // if after 30m since last update, backup can post
      let second_allowed = Clock::current_time_is_strictly_after( 
        self.oracle_timestamp.add_minutes(30i64).unwrap(), 
        TimePrecision::Minute );

      if second_allowed && p.resource_address() == self.oracle2 {
        self.oracle = exch;
        return
      }

      panic!("wrong call")
    }

    // XRD/USD and last time it was updated
    pub fn get_oracle(&self) -> (Decimal, Instant) {
      // TODO once real oracle is running, use the self timestamp
      (self.oracle, Clock::current_time_rounded_to_minutes())
    }
  }
}