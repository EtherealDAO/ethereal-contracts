use scrypto::prelude::*;

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
    // TODO change u32 for VaultId?? to call recall on it?
    power_map: HashMap<ResourceAddress, Vec<u32>>
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

  }

  impl DAO {

    // instantiates the DAO
    // genesis gets delagated all the power 
    pub fn from_nothing(genesis: ComponentAddress) -> ComponentAddress {
      // note to self: allow external superbadge for edao vision
      let dao_superbadge = Vault::with_bucket(ResourceBuilder::new_fungible()
        .mintable(rule!(deny_all), LOCKED)
        .burnable(rule!(deny_all), LOCKED)
        // recall for cleaning up post liquidation
        .recallable(rule!(deny_all), LOCKED)
        .metadata("name", "EDAO SUPERBADGE")
        .mint_initial_supply(1));

      let delegate_resource = 
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
          
          .metadata("name", "EDAO DELEGATE")
          .create_with_no_initial_supply();
      
      let mut power_map = HashMap::new();

      // builtin powers, powers over self
      
      // power 0 -- RETURN TO NOTHING
      // rips dao's soul out and transfers (or destroys) it to a new form
      

      Self {
        dao_superbadge: dao_superbadge,
        power_map: power_map
      }.instantiate().globalize()

    }

    // TODO impl
    // allows superbadge transfer
    // TODO Auth guard only Power 0
    pub fn to_nothing() {

    }

    // TODO impl
    // allows arbitrary change of power map
    // TODO Auth guard only Power 1
    pub fn shift_power() {
      // update the map AND RECALL THE NFT
      // ASSUMPTION BEING THAT AUTHRULES DON'T NEED TO BE UPDATED
    }

    // TODO impl
    // creates the resource, adds it to the map
    // TODO Auth guard only Power 1
    pub fn add_power() {

    }

  }
}