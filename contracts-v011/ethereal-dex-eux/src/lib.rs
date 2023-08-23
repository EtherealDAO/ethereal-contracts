use scrypto::prelude::*;

#[blueprint]
mod eux {
  // OWNER is Alpha
  enable_method_auth! {
    roles {
      alpha => updatable_by: [];
      azero => updatable_by: [];
    },
    methods {
      to_nothing => restrict_to: [azero];
      first_deposit => restrict_to: [alpha];
      start_stop => restrict_to: [alpha];
      add_liquidity => PUBLIC;
      in_given_out => PUBLIC;
      in_given_price => PUBLIC;
      remove_liquidity => PUBLIC;
      sim_swap => PUBLIC;
      spot_price => PUBLIC;
      swap => PUBLIC;
      vault_reserves => PUBLIC;
    }
  }

  struct Eux {
    alpha_addr: ComponentAddress,
    power_eux: Vault,
    pool: (Vault, Vault),
    pool_lp: (ResourceAddress, Decimal),
    swap_fee: Decimal,
    stopped: bool // TODO make work
  }

  impl Eux {
    // 50/50 dao-managed 
    // EUXLP is to be considered a 
    pub fn from_nothing(alpha_addr: ComponentAddress, 
      power_alpha: ResourceAddress, power_azero: ResourceAddress, power_eux: Bucket, 
      t1: ResourceAddress, t2: ResourceAddress,
      swap_fee: Decimal )-> (ComponentAddress, ResourceAddress) {

      // assumed order: EUSD is t1
      // and EXRD is t2
      
      assert!( swap_fee <= dec!("1") && swap_fee >= dec!("0.9"), 
        "fee must be smaller than 10% and positive");

      let lp_ra: ResourceAddress = ResourceBuilder::new_fungible(
          OwnerRole::Fixed(rule!(require(power_eux.resource_address()))))
        .metadata(metadata!(
            init {
                "name" => "Ethereal EUSD/EXRD LP", locked;
                "symbol" => "EUXLP", locked;
            }
        ))
        .burn_roles(burn_roles!(
          burner => rule!(require(power_eux.resource_address()));
          burner_updater => rule!(deny_all);
        ))
        .mint_roles(mint_roles!(
          minter => rule!(require(power_eux.resource_address()));
          minter_updater => rule!(deny_all);
        ))  
        .create_with_no_initial_supply()
        .address();

      let pool = (Vault::new(t1), Vault::new(t2));
      let a1 = Self {
        alpha_addr,
        power_eux: Vault::with_bucket(power_eux),
        pool,
        pool_lp: (lp_ra, dec!(0)),
        swap_fee,
        stopped: false // TODO: need vote to start
      }
      .instantiate()
      .prepare_to_globalize(OwnerRole::None)
      .roles(
        roles!(
          azero => rule!(require(power_azero));
          alpha => rule!(require(power_alpha));
        )
      )
      .globalize()
      .address();

      return (a1, lp_ra)
    }

    // AuthRule: power_zero
    // rips the soul out
    pub fn to_nothing(&mut self) {

    }

    // separated from instantiation for dao reasons
    // separateed from add_liquidity for efficiency reasons
    pub fn first_deposit(&mut self, b1: Bucket, b2: Bucket) -> (Bucket, Option<Bucket>) {
      assert!( !self.stopped && !self.power_eux.is_empty(),
        "DEX stopped or empty"); 

      assert!( self.pool.0.amount() == dec!(0),
        "first deposit into an already running pool");

      self.pool_lp.1 += dec!(10);
      self.pool.0.put(b1);
      self.pool.1.put(b2);

      Self::authorize(&mut self.power_eux, ||
        (ResourceManager::from(self.pool_lp.0).mint(dec!(10)), None)
      )
    }

    // AuthRule: power_alpha
    // full start full stop
    pub fn start_stop(&mut self, input: bool) {
      self.stopped = input;
    }

    // adds all three, basing it on the REAL deposit for correct proportion
    // does not return excess liquidity, just 'swap-balances' them out
    pub fn add_liquidity(&mut self, b1: Bucket, b2: Bucket) -> (Bucket, Option<Bucket>) {
      assert!( !self.stopped && !self.power_eux.is_empty(),
        "DEX stopped or empty"); 

      let amnt1 = b1.amount() / self.pool.0.amount();
      let amnt2 = b2.amount() / self.pool.1.amount();

      if amnt1 > amnt2 {
        let minted = self.pool_lp.1 * amnt1;
        let rem = (dec!(1) -  amnt2 / amnt1) * b1.amount();

        self.pool_lp.1 += minted;

        self.pool.0.put(b1);
        self.pool.1.put(b2);

        return (
          Self::authorize(&mut self.power_eux, 
            || ResourceManager::from(self.pool_lp.0).mint(minted)),
          Some(self.pool.0.take(rem))
        )

      } else if amnt2 > amnt1 {
        let minted = self.pool_lp.1 * amnt1;
        let rem = (dec!(1) -  amnt1 / amnt2) * b2.amount();

        self.pool_lp.1 += minted;

        self.pool.0.put(b1);
        self.pool.1.put(b2);

        return (
          Self::authorize(&mut self.power_eux, 
            || ResourceManager::from(self.pool_lp.0).mint(minted)),
          Some(self.pool.1.take(rem))
        )

      } else {
        let minted = self.pool_lp.1 * amnt1;
        self.pool_lp.1 += minted;

        self.pool.0.put(b1);
        self.pool.1.put(b2);

        return (
          Self::authorize(&mut self.power_eux, 
            || ResourceManager::from(self.pool_lp.0).mint(minted)),
          None
        )
      }
    }

    pub fn remove_liquidity(&mut self, input: Bucket) -> (Bucket, Bucket) {
      // even if stopped or soulless, 
      // can remove liquidity (in equal balance as at time of stop/soulrip)

      assert!( input.resource_address() == self.pool_lp.0,
        "wrong lp resource");

      let per = input.amount() / self.pool_lp.1;
      self.pool_lp.1 -= input.amount();
      Self::authorize(&mut self.power_eux, 
        || ResourceManager::from(self.pool_lp.0).burn(input));

      return (
        self.pool.0.take(self.pool.0.amount() * per), 
        self.pool.1.take(self.pool.1.amount() * per)
      )
    }

    // perform a swap
    fn internal_swap(&mut self, input: Bucket) -> Bucket {
      let size_in = input.amount() * self.swap_fee;
      let ra_in = input.resource_address();

      if ra_in == self.pool.0.resource_address() {
        let size_out = (size_in * self.pool.1.amount()) 
          / (size_in + self.pool.0.amount());

        self.pool.0.put(input);
        self.pool.1.take(size_out)
      } else { // no need to check, will err on wrong ra
        let size_out = (size_in * self.pool.0.amount()) 
          / (size_in + self.pool.1.amount());

        self.pool.1.put(input);
        self.pool.0.take(size_out)
      }
    }

    fn perform_aa(&mut self) {
      let alpha: Global<AnyComponent> = self.alpha_addr.into();

      let (eusd_ca, _, _) = 
        alpha.call_raw::<(ComponentAddress, ComponentAddress, ComponentAddress)>(
          "get_app_addrs", scrypto_args!()
        );
      let eusd: Global<AnyComponent> = eusd_ca.into();

      // assumes the oracle on USD side was rescaled to EXRD from XRD
      if let Some((target, oracle, direction)) = eusd.call_raw::<Option<(Decimal, Decimal, bool)>>
        ("aa_poke", scrypto_args!(self.spot_price())) {
        if let Some(size) = self.in_given_price(target, direction) {

          let input1 = Self::authorize(&mut self.power_eux, || { 
            eusd.call_raw::<Bucket>("aa_woke", scrypto_args!(size, direction))
          });
          let available = input1.amount();

          let mut ret = self.internal_swap(input1);
          
          let profit = if direction {
            // reprice the sold EUSD at the oracle price 
            let repriced = dec!("1") / oracle * available; 

            // profit of treasury, in EXRD
            let mut profit = ret.take(ret.amount() - repriced);
            
            // r1 ~ EUSD
            let r1 = self.internal_swap(profit.take(profit.amount()/dec!("2")));
            let (lp, rem) = self.add_liquidity(profit, r1);
            if let Some(r1p) = rem {
              // rem will be EUSD
              // since the swap for it sold EXRD
              self.pool.0.put(r1p);
            };

            lp
          } else {
            // reprice the sold EXRD at the oracle price 
            let repriced = oracle * available; 

            // profit of treasury, in EUSD
            let mut profit = ret.take(ret.amount() - repriced);

            // r1 ~ EXRD
            let r1 = self.internal_swap(profit.take(profit.amount()/dec!("2")));
            let (lp, rem) = self.add_liquidity(profit, r1);
            if let Some(r1p) = rem {
              // rem will be EXRD
              self.pool.1.put(r1p);
            };

            lp
          };

          eusd.call_raw::<()>("aa_choke", scrypto_args!(ret, profit, direction)); 
        }
      }
    }

    // todo aa_choke cleanup
    pub fn swap(&mut self, input: Bucket) -> Bucket {
      assert!( !self.stopped && !self.power_eux.is_empty(),
        "DEX stopped or empty"); 

      // pre-swap
      self.perform_aa();

      // swap
      let ret = self.internal_swap(input);

      // post-swap
      self.perform_aa();

      return ret
    }

    // internal

    fn authorize<F: FnOnce() -> O, O>(power_eux: &mut Vault, f: F) -> O {
      let temp = power_eux.as_fungible().take_all();
      let ret = temp.authorize_with_all(|| {
        f()
      });
      power_eux.put(temp.into());
      return ret
    }

    // AUXILIARY (for interop)

    // how many to input to get a set number on output? 
    pub fn in_given_out(&self, _output: Decimal, _resource_in: ResourceAddress) { // -> Decimal {

    }

    // how many to input to push it to target price?
    // if direction, sell eusd ~ decrease spot
    // otherwise, sell exrd ~ increase spot
    // returns None, if target < spot
    pub fn in_given_price(&self, target: Decimal, direction: bool) -> Option<Decimal> {
      // sqrt(x * y / target) - x = delta x
      if direction {
        if target < self.spot_price() {
          return Some( 
            ((self.pool.0.amount() * self.pool.1.amount() / target).sqrt().expect("incoherence")
            - self.pool.0.amount()) / self.swap_fee 
          )
        } 
      // sqrt(x * y * target) - y = delta y
      } else {
        if target > self.spot_price() {
          return Some(
            ((self.pool.0.amount() * self.pool.1.amount() * target).sqrt().expect("incoherence")
            - self.pool.1.amount()
            ) / self.swap_fee 
          )
        } 
      }
      return None
    }

    // dumps current # of in each bucket
    pub fn vault_reserves(&self) -> (Decimal, Decimal) {
      (self.pool.0.amount(), self.pool.1.amount())
    }


    // lookup spot price between the assets
    // EUSD / EXRD 
    pub fn spot_price(&self) -> Decimal {
      // amount of exrd increasing means eusd is more valuable
      self.pool.1.amount() / self.pool.0.amount()
    }

    // simulated swap, returns the amount that will be returned with a regular swap
    pub fn sim_swap(&self, _input: Decimal, _resource_in: ResourceAddress) { // -> Decimal {
      
    }
  }
}