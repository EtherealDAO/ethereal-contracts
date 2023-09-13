use scrypto::prelude::*;

#[blueprint]
mod eux {
  enable_method_auth! {
    roles {
      azero => updatable_by: [];
    },
    methods {
      to_nothing => restrict_to: [azero];
      first_deposit => restrict_to: [azero];
      start_stop => restrict_to: [azero];
      add_liquidity => PUBLIC;
      in_given_out => PUBLIC;
      in_given_price => PUBLIC;
      remove_liquidity => PUBLIC;
      sim_swap => PUBLIC;
      spot_price => PUBLIC;
      swap => PUBLIC;
      zap => PUBLIC;
      vault_reserves => PUBLIC;
      look_within => PUBLIC;
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
      power_azero: ResourceAddress, power_eux: Bucket, 
      t1: ResourceAddress, t2: ResourceAddress,
      swap_fee: Decimal, bang: ComponentAddress )-> (ComponentAddress, ResourceAddress) {

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
                "dapp_definitions" =>
                  vec!(GlobalAddress::from(bang)), updatable;
                "icon_url" =>
                  Url::of("https://cdn.discordapp.com/attachments/1092987092864335884/1095874817758081145/logos1.jpeg"),
                  updatable;
                "tags" => vec!["ethereal-dao".to_owned(), "lp".to_owned()], updatable;
                "info_url" => Url::of("https://ethereal.systems"), updatable;
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
              "eux".to_owned()], updatable;
          }
        )
      )
      .globalize()
      .address();

      return (a1, lp_ra)
    }

    // rips the soul and the LP out
    pub fn to_nothing(&mut self) -> (Bucket, Bucket, Bucket) {
      (
        self.power_eux.take_all(),
        self.pool.0.take_all(),
        self.pool.1.take_all()
      )
    }

    pub fn look_within(&self) -> 
      (
        (ResourceAddress, Decimal),
        Decimal,
        bool
      )
    {
      (
        self.pool_lp, 
        self.swap_fee,
        self.stopped
      )
    }

    // separated from instantiation for dao reasons
    // separateed from add_liquidity for efficiency reasons
    pub fn first_deposit(&mut self, b1: Bucket, b2: Bucket) -> (Bucket, Option<Bucket>) {
      assert!( self.pool.0.amount() == dec!(0),
        "first deposit into an already running pool");

      let initmint = (b1.amount() * b2.amount()).checked_sqrt().unwrap();

      self.pool_lp.1 += initmint;
      self.pool.0.put(b1);
      self.pool.1.put(b2);

      Self::authorize(&mut self.power_eux, ||
        (ResourceManager::from(self.pool_lp.0).mint(initmint), None)
      )
    }

    // AuthRule: power_alpha
    // full start full stop
    pub fn start_stop(&mut self, input: bool) {
      self.stopped = input;
    }

    // adds all three, basing it on the REAL deposit for correct proportion
    // does not return excess liquidity, just 'swap-balances' them out
    pub fn add_liquidity(&mut self, mut b1: Bucket, mut b2: Bucket) -> (Bucket, Option<Bucket>) {
      assert!( !self.stopped && !self.power_eux.is_empty(),
        "DEX stopped or empty"); 

      let in1 = b1.amount();
      let in2 = b2.amount();
      let pool1 = self.pool.0.amount();
      let pool2 = self.pool.1.amount();

      if (pool1 / pool2) < (in1 / in2) {
        let in1new = in2 * pool1 / pool2;
        let minted = self.pool_lp.1 * in1new / pool1;

        self.pool_lp.1 += minted;
 
        self.pool.0.put(b1.take(in1new));
        self.pool.1.put(b2);

        return (
          Self::authorize(&mut self.power_eux, 
            || ResourceManager::from(self.pool_lp.0).mint(minted)),
          Some(b1)
        )

      } else if (pool1 / pool2) > (in1 / in2) {
        let in2new = in1 * pool2 / pool1; 
        let minted = self.pool_lp.1 * in2new / pool2;

        self.pool_lp.1 += minted;

        self.pool.0.put(b1);
        self.pool.1.put(b2.take(in2new));

        return (
          Self::authorize(&mut self.power_eux, 
            || ResourceManager::from(self.pool_lp.0).mint(minted)),
          Some(b2)
        )

      } else {
        let minted = self.pool_lp.1 * in1 / pool1;
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
      assert!( !self.stopped && !self.power_eux.is_empty(),
        "DEX stopped or empty"); 

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

    // AA triggers only once per tx, depending on the user direction
    // either letting them buy high / sell low, or being the first to push it down/up
    //
    // ON EXTRAORDINARY HIGH DEPEG: 
    // when the price is pushed with single transactions far beyond the soft peg,
    // due to the system of automatically using profits for LP, it'll take it a few TXs
    // to restore peg, leaving a lot of money on the table
    // Keep in mind this is only a factor when a sigle transaction depegs it by over >10%
    // a couple percents worth of a depeg (<10%) drop in a single transaction, without leaking profit
    // The above was observed with depegs of >300%, 
    // where the profits LP-ing back start to push the price way above peg again
    // At 60% depeg, the profit leak was <5%
    fn perform_aa(&mut self, user_direction: ResourceAddress, 
        first_aa: bool, first_ran: bool) -> bool {
      info!("perform_aa IN");

      // if first performed AA, don't do it twice
      // special case for when user goes from above/beyony peg to the opposite side
      if first_ran {
        return false;
      }

      let alpha: Global<AnyComponent> = self.alpha_addr.into();

      let (eusd_ca, _, _) = 
        alpha.call_raw::<(ComponentAddress, ComponentAddress, ComponentAddress)>(
          "get_app_addrs", scrypto_args!()
        );
      let eusd: Global<AnyComponent> = eusd_ca.into();

      // assumes the oracle on USD side was rescaled to EXRD from XRD
      if let Some((target, oracle, direction)) = eusd.call_raw::<Option<(Decimal, Decimal, bool)>>
        ("aa_poke", scrypto_args!(self.spot_price())) {

        // is the user trying to swap the same direction that the system wants to?
        let aligned_direction = user_direction == if direction 
          { self.pool.0.resource_address() } else 
          { self.pool.1.resource_address() };

        // if the user is swapping against AA, and this is the first AA check
        // then we AA on the second invocation
        // OR
        // if the user is swapping with AA, and this is the second AA check
        // then we have AA'd on the first invocation
        if (!aligned_direction && first_aa) || (aligned_direction && !first_aa) {
          info!("ALIGNED DIRECTION");
          return false
        }

        if let Some(size) = self.in_given_price(target, direction) {

          if let Some(input1) = Self::authorize(&mut self.power_eux, || { 
            eusd.call_raw::<Option<Bucket>>
              ("aa_woke", scrypto_args!(size, direction))
          }) {
            let available = input1.amount();

            let mut ret = self.internal_swap(input1);
            
            let profit = if direction {
              // reprice the sold EUSD at the oracle price 
              let repriced = oracle * available; 
              info!("SOLD {} FOR {} REPRICED AT {}", available, ret.amount(), repriced);

              // profit of treasury, in EXRD
              let mut profit = ret.take(ret.amount() - repriced);
              info!("AA PROFIT: TOOK {}", profit.amount());
              // r1 ~ EUSD
              let r1 = self.internal_swap(profit.take(profit.amount()/dec!("2")));
              info!("AA PROFIT: SWAPPED HALF FOR {}", r1.amount());
              let (lp, rem) = self.add_liquidity(r1, profit);
              info!("AA PROFIT: ADDED LIQUIDITY");
              if let Some(r1p) = rem {
                // yes
                if self.pool.0.resource_address() == r1p.resource_address() {
                  self.pool.0.put(r1p);
                } else {
                  self.pool.1.put(r1p);
                }
              };
              info!("RETURNING LP");

              lp
            } else {
              // reprice the sold EXRD at the oracle price 
              let repriced = dec!("1") / oracle * available; 

              // profit of treasury, in EUSD
              let mut profit = ret.take(ret.amount() - repriced);

              // r1 ~ EXRD
              let r1 = self.internal_swap(profit.take(profit.amount()/dec!("2")));
              let (lp, rem) = self.add_liquidity(profit, r1);
              if let Some(r1p) = rem {
                if self.pool.0.resource_address() == r1p.resource_address() {
                  self.pool.0.put(r1p);
                } else {
                  self.pool.1.put(r1p);
                }
              };

              lp
            };
            info!("perform_aa OUT");

            eusd.call_raw::<()>("aa_choke", scrypto_args!(ret, profit, direction)); 
            return true 
          }
        }
      }
      return false
    }

    pub fn swap(&mut self, input: Bucket) -> Bucket {
      assert!( !self.stopped && !self.power_eux.is_empty(),
        "DEX stopped or empty"); 

      let direction = input.resource_address();

      // pre-swap
      let ran = self.perform_aa(direction, true, false);

      // swap
      let ret = self.internal_swap(input);

      // post-swap
      self.perform_aa(direction, false, ran);

      return ret
    }

    // EXRD | EUSD -> EUXLP
    // I am well aware that this isn't the exact equation
    // but I am willing to ignore it
    pub fn zap(&mut self, mut input: Bucket) -> Bucket {
      assert!( !self.stopped && !self.power_eux.is_empty(),
        "DEX stopped or empty"); 

      let direction = input.resource_address();

      // pre-swap
      let ran = self.perform_aa(direction, true, false);

      // ghetto zap
      //  if it's good enough for AA, it's good enough for you
      let p2 = self.internal_swap(input.take(input.amount()/dec!(2)));
      let (ret, rem) = if input.resource_address() == self.pool.0.resource_address() {
        self.add_liquidity(input, p2)
      } else {
        self.add_liquidity(p2, input)
      };

      if let Some(r1p) = rem {
        if self.pool.0.resource_address() == r1p.resource_address() {
          self.pool.0.put(r1p);
        } else {
          self.pool.1.put(r1p);
        }
      };

      // post-swap
      self.perform_aa(direction, false, ran);

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
            ((self.pool.0.amount() * self.pool.1.amount() / target).checked_sqrt().expect("incoherence")
            - self.pool.0.amount()) / self.swap_fee 
          )
        } 
      // sqrt(x * y * target) - y = delta y
      } else {
        if target > self.spot_price() {
          return Some(
            ((self.pool.0.amount() * self.pool.1.amount() * target).checked_sqrt().expect("incoherence")
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