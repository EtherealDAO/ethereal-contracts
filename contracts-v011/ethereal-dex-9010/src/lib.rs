use scrypto::prelude::*;
use scrypto_math::*;

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
    }
  }

  struct Dex {
    power_alpha: ResourceAddress,
    power_dex: Vault,
    resources: ((ResourceAddress, Decimal), (ResourceAddress, Decimal)),
    pool: Global<TwoResourcePool>,
    swap_fee: Decimal,
    stopped: bool
  }

  impl Dex {
    // instantiates the TriPool, 
    // starting it as 'stopped'
    pub fn from_nothing(power_alpha: ResourceAddress, power_dex: Bucket, 
      t1: ResourceAddress, t1w: Decimal, t2: ResourceAddress, t2w: Decimal,
      swap_fee: Decimal )-> Global<Dex> {
     
      assert!( t1w + t2w == dec!("1") && t1w > dec!("0") && t2w > dec!("0"), 
        "weights must sum to 1 and both be positive");
      
      assert!( swap_fee < dec!("0.1") && swap_fee >= dec!("0"), 
        "fee must be smaller than 10% and positive");

      let pool = Blueprint::<TwoResourcePool>::instantiate(
        OwnerRole::Fixed(rule!(require(power_alpha))),
        rule!(require(power_dex.resource_address())),
        (t1, t2),
      );

      Self {
        power_alpha,
        power_dex: Vault::with_bucket(power_dex),
        resources: ((t1, t1w), (t2, t2w)),
        pool,
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


    // .metadata(metadata!(
    //   init {
    //     "name" => "Ethereal TriPoolLP", locked;
    //     "symbol" => "TLP", locked;
    //     "url" => "todo", locked;
    //     "image" => "todo", locked;
    //   }
    // ))

    // AuthRule: power_zero
    // rips the soul out
    // the TriPool's TLP is managed by the native component
    // so the liquidity is left alone
    pub fn to_nothing(&mut self) -> Bucket {
      self.power_dex.take_all()
    }

    // separated from instantiation for dao reasons
    // separateed from add_liquidity for efficiency reasons
    pub fn first_deposit(&mut self, b1: Bucket, b2: Bucket) -> (Bucket, Option<Bucket>) {
      assert!( !self.stopped && !self.power_dex.is_empty(),
        "DEX stopped or empty"); 

      assert!( *self.vault_reserves().iter().next().expect("incoherence").1 == dec!(0),
        "first deposit into an already running pool");

      Self::authorize(&mut self.power_dex, ||
        self.pool.contribute((b1, b2))
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

      Self::authorize(&mut self.power_dex, ||
        self.pool.contribute((b1, b2))
      )
    }

    pub fn remove_liquidity(&mut self, input: Bucket) -> (Bucket, Bucket) {
      // even if stopped or soulless, 
      // can remove liquidity (in equal balance as at time of stop/soulrip)

      self.pool.redeem(input)
    }

    // no slippage limit, can set it in the manifest
    pub fn swap(&mut self, input: Bucket) -> Bucket {
      assert!( !self.stopped && !self.power_dex.is_empty(),
        "DEX stopped or empty"); 

      let size_in = input.amount() * (dec!("1") - self.swap_fee);
      let ra_in = input.resource_address();

      let reserves = self.vault_reserves();

      let (ra_out, w_out) = if ra_in == self.resources.0.0 {
        self.resources.1
      } else if ra_in == self.resources.1.0 {
        self.resources.0
      } else {
        panic!("wrong resource input")
      };

      let reserves_out = reserves.get(&ra_out).expect("coherence error");
      let reserves_in = reserves.get(&ra_in).expect("coherence error");

      let size_out = 
        *reserves_out * (dec!("1") - 
          (*reserves_in / (*reserves_in + size_in))
            .pow((dec!("1") - w_out) / w_out).expect("power incoherence") 
        );

      Self::authorize(&mut self.power_dex, || {
        self.pool.protected_deposit(input);
        self.pool.protected_withdraw(ra_out, size_out, 
          WithdrawStrategy::Rounded(RoundingMode::ToZero))
      })
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

    pub fn change_fee(&mut self, new_fee: Decimal) {

    }

    // AUXILIARY (for interop)

    // how many to input to get a set number on output? 
    pub fn in_given_out(&self, _output: Decimal, _resource_in: ResourceAddress) { // -> Decimal {

    }

    // how many to input to push it to target price?
    // returns None, if target < spot
    pub fn in_given_price(&self, _target: Decimal, _resource_in: ResourceAddress) { // -> Option<Decimal> {

    }

    // dumps current # of in each bucket
    pub fn vault_reserves(&self) -> BTreeMap<ResourceAddress, Decimal> {
      self.pool.get_vault_amounts()
    }


    // lookup spot price between the assets
    pub fn spot_price(&self) -> Decimal {
      let reserves = self.vault_reserves();

      ((reserves.get(self.resources.0.0).expect("incoherence") / self.resources.0.1)
      /
      (reserves.get(self.resources.1.0).expect("incoherence") / self.resources.1.1))
      *
      (dec!("1") / (dec!("1") - self.swap_fee))
    }

    // simulated swap, returns the amount that will be returned with a regular swap
    pub fn sim_swap(&self, _input: Decimal, _resource_in: ResourceAddress) { // -> Decimal {

    }
  }
}