use scrypto::prelude::*;

external_component! {
  Dao {
    fn get_branch_addrs(&self) -> (ComponentAddress, ComponentAddress, ComponentAddress);
    fn vote(&mut self, vote: bool, proposal: u64, proof: Proof);
  }
}

external_component! {
  Usd {
    fn tri_check(&mut self, )
  }
}

// The TriPool dex
// V1, only a single pair. 
// Hardcoded Balancer V1 type pool with no ability to alter weights
#[blueprint]
mod dex {
  struct Dex {
    alpha_addr: ComponentAddress,
    power_alpha: ResourceAddress,
    power_zero: ResourceAddress,

    power_dex: Vault,

    // $REAL, $eUSD, $eXRD
    // and their weights
    pools: Vec<(Decimal, Vault)>,
    lp_resource: ResourceAddress,
    total_lp: Decimal,

    invariant: Decimal,

    swap_fee: Decimal,

    stopped: bool,
  }

  impl Dex {
    // instantiates the TriPool, 
    // starting it as 'stopped'
    pub fn from_nothing() -> ComponentAddress {

      let lp_resource = ResourceBuilder::new_fungible()
        .metadata("symbol", "TLP")
        .metadata("name", "Ethereal TriPoolLP")
        .metadata("url", "todo")
        .metadata("image", "todo")
        .mintable(rule!(require(power_dex.resource_address())), LOCKED)
        .burnable(rule!(require(power_dex.resource_address())), LOCKED)
        .create_with_no_initial_supply();

      let invariant = dec(1);

      let acc_rules = 
        AccessRulesConfig::new()
          .method("to_nothing", rule!(require(power_zero)), LOCKED)
          .method("start_stop", rule!(require(power_alpha)), LOCKED)
          .default(rule!(allow_all), LOCKED);
    }

    // AuthRule: power_zero
    // rips the soul (and the liquidty) out
    pub fn to_nothing(&mut self) -> (Bucket, Bucket, Bucket, Bucket) {

    }

    // separated from instantiation for dao reasons
    // separateed from add_liquidity for efficiency reasons
    pub fn first_deposit(&mut self, real: Bucket, eusd: Bucket, exrd: Bucket) -> Bucket {
      assert!( !self.stopped && !self.power_dex.is_empty(),
        "DEX stopped or empty"); 

      asert!( self.pools[0].1.is_empty(), 
        "first deposit into an already running pool");

      self.pools[0].1.put(real);
      self.pools[1].1.put(eusd);
      self.pools[2].1.put(exrd);
    }

    // AuthRule: power_alpha
    // full start full stop
    pub fn start_stop(&mut self, input: bool) {
      self.stopped = input;
    }

    // adds all three, basing it on the REAL deposit for correct proportion
    // does not return excess liquidity, just 'swap-balances' them out
    pub fn add_liquidity(&mut self, real: Bucket, eusd: Bucket, exrd: Bucket) {
      assert!( !self.stopped && !self.power_dex.is_empty(),
        "DEX stopped or empty"); 
    }

    // deposits a single asset, moving prices of everything
    pub fn sa_add_liquidity(&mut self, input: Bucket, index: u64) -> Bucket {
      assert!( !self.stopped && !self.power_dex.is_empty(),
        "DEX stopped or empty"); 

      let size_in = input.amount();
      let mut pool = self.pools[index as usize].1;
      let pool_amnt = pool.amount();
      let weight = self.pools[index as usize].2;

      let lp = self.total_lp * (1 + (size_in / pool_amnt)) // .  pow(weight) - 1)
      self.total_lp += lp;

      // todo get a real decimal to decimal power function
      self.invariant *= ((pool_amnt + size_in) / pool_amnt); // .pow(weight) ;      
      pool.put(input);

      self.power_dex.authorize(||
        borrow_resource_manager!(self.lp_resource)
          .mint(lp)
      )
    }

    pub fn remove_liquidity(&mut self, input: Bucket) -> (Bucket, Bucket, Bucket) {
      // even if stopped or soulless, 
      // can remove liquidity (in equal balance as at time of stop/soulrip)

      // TODO: fee
    }

    pub fn sa_remove_liquidity(&mut self, input: Bucket, index: u64) -> Bucket {
      assert!( !self.stopped && !self.power_dex.is_empty(),
        "DEX stopped or empty"); 

      let size_in = input.amount();
      let lp_total = self.total_lp;
      
      let out = self.pools[index as usize].1.amount() * (1 - (1 - (size_in / lp_total))); // .pow(1 / weight) )); 

    }

    pub fn swap(&mut self, input: Bucket, index_in: u64, index_out: u64) {
      assert!( !self.stopped && !self.power_dex.is_empty(),
        "DEX stopped or empty"); 

      let size_in = input.amount();

    }

    // AUXILIARY (for interop)

    // how many to input to get a set number on output?
    pub fn in_given_out(&self, output: Decimal, index_in: u64, index_out: u64) -> Decimal {

    }

    // how many to input to push it to target price?
    // returns None, if target < spot
    pub fn in_given_price(&self, target: Decimal, index_in: u64, index_out: u64) -> Option<Decimal> {

    }

    // dumps current # of in each bucket
    pub fn check_pools(&self) -> (Decimal, Decimal, Decimal) {

    }

    // lookup spot price between the assets
    pub fn spot_price(&self, index_in: u64, index_out: u64) -> Decimal {

    }

    // simulated swap, returns the amount that will be returned with a regular swap
    pub fn sim_swap(&self, input: Decimal, index_in: u64, index_out: u64) -> Decimal {

    }
  }
}