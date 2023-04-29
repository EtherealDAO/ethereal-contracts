use scrypto::prelude::*;

external_component! {
  Genesis {
      fn deposit(&mut self, input: Bucket);
  }
}

#[derive(ScryptoSbor, NonFungibleData)]
pub struct DelegateBadge {
    id: u64,
}

// the DAO blueprint manages a set of components
// it is concerned with 
// 1) tracking and updating parameters to them
// 2) replacing them 
// 3) replacing itself
// 4) holding protocol badges + authenticating their use

#[blueprint]
mod dao {
  struct DAO {
    // alpha Delta and omega
    // the authority on everything
    dao_superbadge: Vault,

    // from power index to a pair of 
    // delegate resource + list of delegate badges
    // uses delegate resource
    // TODO change u64 for VaultId?? 
    // to call recall on it? needs recall updates
    power_map: 
      HashMap<
        ResourceAddress, 
        Vec<u64>>,
    // TODO need to encode how to update
    // store a pointer? if it can work
    // ...
    // can it just work RAW by calling 
    //  add_access_check on comps
    // and 
    // set_X on resources?
    // should work!    
    // ...
    // or just don't update? 
    // much easier but NEEDS recall capability

    // tracks the ID of the last delegate
    // TODO use the localID? currently there's some decode/encode errors
    delegate_id: u64,

    // tracks the builtin powers over itself
    // could rely on ordering of power map instead
    // but then power removal becomes problematic
    power_zero: ResourceAddress,
    power_one: ResourceAddress
  }

  impl DAO {

    // instantiates the DAO
    // genesis gets delagated all the power 
    // genesis is for sake of simplicity started as a sole admin
    // it needs a 'deposit' method to take in the power badge
    pub fn from_nothing(genesis: ComponentAddress) -> ComponentAddress {
      // note to self: allow external superbadge for edao vision
      let dao_superbadge = Vault::with_bucket(ResourceBuilder::new_fungible()
        .mintable(rule!(deny_all), LOCKED)
        .burnable(rule!(deny_all), LOCKED)
        // recall for cleaning up post liquidation
        .recallable(rule!(deny_all), LOCKED)
        .metadata("name", "EDAO SUPERBADGE")
        // TODO add name param?
        .mint_initial_supply(1));
      
      let mut power_map = HashMap::new();
      let mut delegate_id = 0;

      // builtin powers, powers over self
      
      // Power 0 -- RETURN TO NOTHING
      // rips dao's soul out and transfers (or destroys) it to a new form
      // used to transform it to a new form while retaining all of the resources
      let power_zero = 
        ResourceBuilder::new_uuid_non_fungible::<DelegateBadge>()
          .mintable(
            rule!(require(dao_superbadge.resource_address())), LOCKED)
          .burnable(
            rule!(require(dao_superbadge.resource_address())), LOCKED)
          // recall for cleaning up old badges
          .recallable(
            rule!(require(dao_superbadge.resource_address())), LOCKED)
          .restrict_withdraw(
            rule!(require(dao_superbadge.resource_address())), LOCKED)
          .restrict_deposit(
            rule!(require(dao_superbadge.resource_address())), LOCKED)
          .metadata("name", "EDAO POWER ZERO")
          .create_with_no_initial_supply();
      
      power_map.insert(power_zero, vec![]).unwrap();

      // Power 1 -- MONOPOLY OVER VIOLENCE
      // exerts the dao's power over internal powers
      // adding, removing, or changing their structure 
      let power_one = 
        ResourceBuilder::new_uuid_non_fungible::<()>()
          .mintable(
            rule!(require(dao_superbadge.resource_address())), LOCKED)
          .burnable(
            rule!(require(dao_superbadge.resource_address())), LOCKED)
          // recall for cleaning up old badges
          .recallable(
            rule!(require(dao_superbadge.resource_address())), LOCKED)
          .restrict_withdraw(
            rule!(require(dao_superbadge.resource_address())), LOCKED)
          .restrict_deposit(
            rule!(require(dao_superbadge.resource_address())), LOCKED)
          .metadata("name", "EDAO POWER ONE")
          .create_with_no_initial_supply();

      // genesis gets the initial power one delegation
      Genesis::at(genesis).deposit(
        dao_superbadge.authorize(|| 
          borrow_resource_manager!(power_one)
            .mint_uuid_non_fungible(DelegateBadge { id: delegate_id } )
          )
        );
      power_map.insert(power_one, vec![delegate_id]).unwrap();

      delegate_id += 1;

      let acc_rules = 
        AccessRulesConfig::new()
          .method("to_nothing", rule!(require(power_zero)), LOCKED)
          .method("shift_power", rule!(require(power_one)), LOCKED)
          .method("add_power", rule!(require(power_one)), LOCKED)
          .method("remove_power", rule!(require(power_one)), LOCKED)
          .default(rule!(allow_all), LOCKED);

      Self {
        dao_superbadge: dao_superbadge,
        power_map: power_map,
        delegate_id: delegate_id,
        power_zero: power_zero,
        power_one: power_one
      }
      .instantiate()
      .globalize_with_access_rules(acc_rules)
    }

    // TODO impl
    // allows superbadge transfer
    pub fn to_nothing() {

    }

    // TODO impl
    // allows arbitrary change of power map
    pub fn shift_power() {
      // update the map AND RECALL THE NFT
      // ASSUMPTION BEING THAT AUTHRULES DON'T NEED TO BE UPDATED
    }

    // TODO impl
    // creates the resource, adds it to the map
    pub fn add_power() {

    }

    // TODO impl
    // destroys the resource, removes it from the map
    // only works if the delegates are empty
    pub fn remove_power() {

    }

  }
}