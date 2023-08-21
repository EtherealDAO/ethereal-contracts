use scrypto::prelude::*;

#[blueprint]
mod dex {
  // OWNER is Alpha
  enable_method_auth! {
    roles {
      alpha => updatable_by: [];
    },
    methods {
      to_nothing => restrict_to: [alpha];
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
      mock_change_eusd => PUBLIC;
    }
  }

  struct Dex {
    eusd_addr: ComponentAddress,
    power_dex: Vault,
    pool: (Vault, Vault),
    pool_lp: (ResourceAddress, Decimal),
    swap_fee: Decimal,
    stopped: bool
  }

  impl Dex {
    // 50/50 dao-managed 
    // EUXLP is to be considered a 
    pub fn from_nothing(eusd_addr: ComponentAddress, 
      power_alpha: ResourceAddress, power_dex: Bucket, 
      t1: ResourceAddress, t2: ResourceAddress,
      swap_fee: Decimal )-> Global<Dex> {

      // assumed order: EUSD is t1
      // and EXRD is t2
      
      assert!( swap_fee <= dec!("1") && swap_fee >= dec!("0.9"), 
        "fee must be smaller than 10% and positive");

      let lp_ra: ResourceAddress = ResourceBuilder::new_fungible(
          OwnerRole::Fixed(rule!(require(power_dex.resource_address()))))
        .metadata(metadata!(
            init {
                "name" => "Ethereal EUSD/EXRD LP", locked;
                "symbol" => "EUXLP", locked;
            }
        ))
        .burn_roles(burn_roles!(
          burner => rule!(require(power_dex.resource_address()));
          burner_updater => rule!(deny_all);
        ))
        .mint_roles(mint_roles!(
          minter => rule!(require(power_dex.resource_address()));
          minter_updater => rule!(deny_all);
        ))  
        .create_with_no_initial_supply()
        .address();

      let pool = (Vault::new(t1), Vault::new(t2));
      Self {
        eusd_addr,
        power_dex: Vault::with_bucket(power_dex),
        pool,
        pool_lp: (lp_ra, dec!(0)),
        swap_fee,
        stopped: false
      }
      .instantiate()
      .prepare_to_globalize(OwnerRole::None)
      .roles(
        roles!(
          alpha => rule!(require(power_alpha));
        )
      )
      .globalize()
    }

    // AuthRule: power_zero
    // rips the soul out
    pub fn to_nothing(&mut self) {

    }

    // separated from instantiation for dao reasons
    // separateed from add_liquidity for efficiency reasons
    pub fn first_deposit(&mut self, b1: Bucket, b2: Bucket) -> (Bucket, Option<Bucket>) {
      assert!( !self.stopped && !self.power_dex.is_empty(),
        "DEX stopped or empty"); 

      assert!( self.pool.0.amount() == dec!(0),
        "first deposit into an already running pool");

      self.pool_lp.1 += dec!(10);
      self.pool.0.put(b1);
      self.pool.1.put(b2);

      Self::authorize(&mut self.power_dex, ||
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
      assert!( !self.stopped && !self.power_dex.is_empty(),
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
          Self::authorize(&mut self.power_dex, 
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
          Self::authorize(&mut self.power_dex, 
            || ResourceManager::from(self.pool_lp.0).mint(minted)),
          Some(self.pool.1.take(rem))
        )

      } else {
        let minted = self.pool_lp.1 * amnt1;
        self.pool_lp.1 += minted;

        self.pool.0.put(b1);
        self.pool.1.put(b2);

        return (
          Self::authorize(&mut self.power_dex, 
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
      Self::authorize(&mut self.power_dex, 
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

    fn perform_aa(&mut self) -> Option<(Bucket, Option<Bucket>)> {
      let eusd: Global<AnyComponent> = self.eusd_addr.into();
      if let Some((target, direction)) = eusd.call_raw::<Option<(Decimal, bool)>>
        ("aa_poke", scrypto_args!(self.spot_price())) {
        if let Some(sizehalf) = self.in_given_price(target, direction) {

          let mut input1 = Self::authorize(&mut self.power_dex, || { 
            // todo make it an alpha lookup
            eusd.call_raw::<Bucket>("aa_woke", scrypto_args!(sizehalf, direction))
          });
          let input2 = self.internal_swap(input1.take(sizehalf));
          let (out, rem) = if direction {
            self.add_liquidity(input1, input2)
          } else {
            self.add_liquidity(input2, input1)
          };

          return Some((out, rem)) // todo aa_choke
        }
      }
      return None
    }

    // todo aa_choke cleanup
    pub fn swap(&mut self, input: Bucket) -> (Bucket, Option<(Bucket, Option<Bucket>)>, Option<(Bucket, Option<Bucket>)>) {
      assert!( !self.stopped && !self.power_dex.is_empty(),
        "DEX stopped or empty"); 

      // pre-swap
      let ret2 = self.perform_aa();

      // swap
      let ret = self.internal_swap(input);

      // post-swap
      let ret3 = self.perform_aa();

      return (ret, ret2, ret3)
    }

    // internal

    fn authorize<F: FnOnce() -> O, O>(power_dex: &mut Vault, f: F) -> O {
      let temp = power_dex.as_fungible().take_all();
      let ret = temp.authorize_with_all(|| {
        f()
      });
      power_dex.put(temp.into());
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

    pub fn mock_change_eusd(&mut self, eusd: ComponentAddress) {
      self.eusd_addr = eusd;
    }
  }
}