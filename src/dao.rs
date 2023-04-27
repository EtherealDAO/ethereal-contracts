use scrypto::prelude::*;

// the DAO blueprint manages a set of components
// it is concerned with 
// 1) tracking and updating parameters to them
// 2) replacing them 
// 3) replacing itself
// 4) holding protocol badges + authenticating their use

#[blueprint]
mod dao {
  struct Dao {
    // alpha Delta and omega
    // the authority on everything
    dao_superbadge: Vault,

    // delegate resource
    delegate_resource: ResourceAddress,

    // next power index
    power_id: u32,

    // from power index to a pair of 
    // superauth badge + list of delegate badges
    // uses delegate resource
    power_map: HashMap<u32, (ResourceAddress, Vec<u32>)>
    // TODO need to encode how to update
    // store a pointer? if it can work
    // ...
    // can it just work RAW by calling 
    //  add_access_check on comps
    // and 
    // set_X on resources?
    // should work!

  }

  impl Dao {

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
          .metadata("name", "EDAO DELEGATE")
          .create_with_no_initial_supply();
      
      let mut power_id: u32 = 0; 



      Self {
        dao_superbadge: dao_superbadge,
        delegate_resource: owner_resource,
        power_id: 0,
        power_map: HashMap::new()
      }.instantiate().globalize()

    }

  }
}