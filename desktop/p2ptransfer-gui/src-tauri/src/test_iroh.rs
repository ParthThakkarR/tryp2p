use iroh::endpoint::Endpoint;
use iroh::discovery::pkarr::PkarrPublisher;
use iroh::discovery::local_swarm_discovery::LocalSwarmDiscovery;

pub fn test() {
    let _x = LocalSwarmDiscovery::new("test".parse().unwrap());
}
