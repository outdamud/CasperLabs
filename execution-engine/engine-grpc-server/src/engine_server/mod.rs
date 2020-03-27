include!(concat!(
    env!("OUT_DIR"),
    "/../../../../generated_protobuf/ipc.rs"
));
include!(concat!(
    env!("OUT_DIR"),
    "/../../../../generated_protobuf/ipc_grpc.rs"
));
include!(concat!(
    env!("OUT_DIR"),
    "/../../../../generated_protobuf/state.rs"
));
include!(concat!(
    env!("OUT_DIR"),
    "/../../../../generated_protobuf/transforms.rs"
));
pub mod mappings;

use std::{
    collections::BTreeMap,
    convert::{TryFrom, TryInto},
    fmt::Debug,
    io::ErrorKind,
    iter::FromIterator,
    marker::{Send, Sync},
    time::Instant,
};

use grpc::{Error as GrpcError, RequestOptions, ServerBuilder, SingleResponse};
use log::{info, warn, Level};

use engine_core::engine_state::{
    execute_request::ExecuteRequest,
    genesis::{GenesisConfig, GenesisResult},
    query::{QueryRequest, QueryResult},
    upgrade::{UpgradeConfig, UpgradeResult},
    EngineState, Error as EngineError,
};
use engine_shared::{
    logging::{self, log_duration},
    newtypes::{Blake2bHash, CorrelationId},
};
use engine_storage::global_state::{CommitResult, StateProvider};
use types::{bytesrepr::ToBytes, ProtocolVersion};

use self::{
    ipc::{
        BidStateRequest, BidStateResponse, ChainSpec_GenesisConfig, CommitRequest, CommitResponse,
        DistributeRewardsRequest, DistributeRewardsResponse, ExecuteResponse, GenesisResponse,
        QueryResponse, SlashRequest, SlashResponse, UnbondPayoutRequest, UnbondPayoutResponse,
        UpgradeRequest, UpgradeResponse,
    },
    ipc_grpc::{ExecutionEngineService, ExecutionEngineServiceServer},
    mappings::{ParsingError, TransformMap},
};

const METRIC_DURATION_COMMIT: &str = "commit_duration";
const METRIC_DURATION_EXEC: &str = "exec_duration";
const METRIC_DURATION_QUERY: &str = "query_duration";
const METRIC_DURATION_GENESIS: &str = "genesis_duration";
const METRIC_DURATION_UPGRADE: &str = "upgrade_duration";

const TAG_RESPONSE_COMMIT: &str = "commit_response";
const TAG_RESPONSE_EXEC: &str = "exec_response";
const TAG_RESPONSE_QUERY: &str = "query_response";
const TAG_RESPONSE_GENESIS: &str = "genesis_response";
const TAG_RESPONSE_UPGRADE: &str = "upgrade_response";

const UNIMPLEMENTED: &str = "unimplemented";

const DEFAULT_PROTOCOL_VERSION: ProtocolVersion = ProtocolVersion::V1_0_0;

// Idea is that Engine will represent the core of the execution engine project.
// It will act as an entry point for execution of Wasm binaries.
// Proto definitions should be translated into domain objects when Engine's API
// is invoked. This way core won't depend on casperlabs-engine-grpc-server
// (outer layer) leading to cleaner design.
impl<S> ExecutionEngineService for EngineState<S>
where
    S: StateProvider,
    EngineError: From<S::Error>,
    S::Error: Into<engine_core::execution::Error> + Debug,
{
    fn query(
        &self,
        _request_options: RequestOptions,
        query_request: ipc::QueryRequest,
    ) -> SingleResponse<QueryResponse> {
        let start = Instant::now();
        let correlation_id = CorrelationId::new();

        let request: QueryRequest = match query_request.try_into() {
            Ok(ret) => ret,
            Err(err) => {
                let log_message = format!("{:?}", err);
                warn!("{}", log_message);
                let mut result = ipc::QueryResponse::new();
                result.set_failure(log_message);
                log_duration(
                    correlation_id,
                    METRIC_DURATION_QUERY,
                    TAG_RESPONSE_QUERY,
                    start.elapsed(),
                );
                return SingleResponse::completed(result);
            }
        };

        let result = self.run_query(correlation_id, request);

        let response = match result {
            Ok(QueryResult::Success(value)) => {
                let mut result = ipc::QueryResponse::new();
                match value.to_bytes() {
                    Ok(serialized_value) => {
                        info!("query successful; correlation_id: {}", correlation_id);
                        result.set_success(serialized_value);
                    }
                    Err(error_msg) => {
                        let log_message = format!("Failed to serialize StoredValue: {}", error_msg);
                        warn!("{}", log_message);
                        result.set_failure(log_message);
                    }
                }
                result
            }
            Ok(QueryResult::ValueNotFound(msg)) => {
                info!("{}", msg);
                let mut result = ipc::QueryResponse::new();
                result.set_failure(msg);
                result
            }
            Ok(QueryResult::RootNotFound) => {
                let log_message = "Root not found";
                info!("{}", log_message);
                let mut result = ipc::QueryResponse::new();
                result.set_failure(log_message.to_string());
                result
            }
            Ok(QueryResult::CircularReference(msg)) => {
                warn!("{}", msg);
                let mut result = ipc::QueryResponse::new();
                result.set_failure(msg);
                result
            }
            Err(err) => {
                let log_message = format!("{:?}", err);
                warn!("{}", log_message);
                let mut result = ipc::QueryResponse::new();
                result.set_failure(log_message);
                result
            }
        };

        log_duration(
            correlation_id,
            METRIC_DURATION_QUERY,
            TAG_RESPONSE_QUERY,
            start.elapsed(),
        );

        SingleResponse::completed(response)
    }

    fn execute(
        &self,
        _request_options: RequestOptions,
        exec_request: ipc::ExecuteRequest,
    ) -> SingleResponse<ExecuteResponse> {
        let start = Instant::now();
        let correlation_id = CorrelationId::new();

        let exec_request: ExecuteRequest = match exec_request.try_into() {
            Ok(ret) => ret,
            Err(err) => {
                return SingleResponse::completed(err);
            }
        };

        let mut exec_response = ExecuteResponse::new();

        let results = match self.run_execute(correlation_id, exec_request) {
            Ok(results) => results,
            Err(error) => {
                info!("deploy results error: RootNotFound");
                exec_response.mut_missing_parent().set_hash(error.to_vec());
                log_duration(
                    correlation_id,
                    METRIC_DURATION_EXEC,
                    TAG_RESPONSE_EXEC,
                    start.elapsed(),
                );
                return SingleResponse::completed(exec_response);
            }
        };

        let protobuf_results_iter = results.into_iter().map(Into::into);
        exec_response
            .mut_success()
            .set_deploy_results(FromIterator::from_iter(protobuf_results_iter));
        log_duration(
            correlation_id,
            METRIC_DURATION_EXEC,
            TAG_RESPONSE_EXEC,
            start.elapsed(),
        );
        SingleResponse::completed(exec_response)
    }

    fn commit(
        &self,
        _request_options: RequestOptions,
        mut commit_request: CommitRequest,
    ) -> SingleResponse<CommitResponse> {
        let start = Instant::now();
        let correlation_id = CorrelationId::new();

        // TODO
        let protocol_version = {
            let protocol_version = commit_request.take_protocol_version().into();
            if protocol_version < DEFAULT_PROTOCOL_VERSION {
                DEFAULT_PROTOCOL_VERSION
            } else {
                protocol_version
            }
        };

        // Acquire pre-state hash
        let pre_state_hash: Blake2bHash = match commit_request.get_prestate_hash().try_into() {
            Err(_) => {
                let error_message = "Could not parse pre-state hash".to_string();
                warn!("{}", error_message);
                let mut commit_response = CommitResponse::new();
                commit_response
                    .mut_failed_transform()
                    .set_message(error_message);
                return SingleResponse::completed(commit_response);
            }
            Ok(hash) => hash,
        };

        // Acquire commit transforms
        let transforms = match TransformMap::try_from(commit_request.take_effects().into_vec()) {
            Err(ParsingError(error_message)) => {
                warn!("{}", error_message);
                let mut commit_response = CommitResponse::new();
                commit_response
                    .mut_failed_transform()
                    .set_message(error_message);
                return SingleResponse::completed(commit_response);
            }
            Ok(transforms) => transforms.into_inner(),
        };

        // "Apply" effects to global state
        let commit_response = {
            let mut ret = CommitResponse::new();

            match self.apply_effect(correlation_id, protocol_version, pre_state_hash, transforms) {
                Ok(CommitResult::Success {
                    state_root,
                    bonded_validators,
                }) => {
                    let properties = {
                        let mut tmp = BTreeMap::new();
                        tmp.insert("post-state-hash", format!("{:?}", state_root));
                        tmp.insert("success", true.to_string());
                        tmp
                    };
                    logging::log_details(
                        Level::Info,
                        "effects applied; new state hash is: {post-state-hash}".to_owned(),
                        properties,
                    );

                    let bonds = bonded_validators.into_iter().map(Into::into).collect();
                    let commit_result = ret.mut_success();
                    commit_result.set_poststate_hash(state_root.to_vec());
                    commit_result.set_bonded_validators(bonds);
                }
                Ok(CommitResult::RootNotFound) => {
                    warn!("RootNotFound");
                    ret.mut_missing_prestate().set_hash(pre_state_hash.to_vec());
                }
                Ok(CommitResult::KeyNotFound(key)) => {
                    warn!("{:?} not found", key);
                    ret.set_key_not_found(key.into());
                }
                Ok(CommitResult::TypeMismatch(type_mismatch)) => {
                    warn!("{:?}", type_mismatch);
                    ret.set_type_mismatch(type_mismatch.into());
                }
                Ok(CommitResult::Serialization(error)) => {
                    warn!("{:?}", error);
                    ret.mut_failed_transform()
                        .set_message(format!("{:?}", error));
                }
                Err(error) => {
                    warn!("State error {:?} when applying transforms", error);
                    ret.mut_failed_transform()
                        .set_message(format!("{:?}", error));
                }
            }

            ret
        };

        log_duration(
            correlation_id,
            METRIC_DURATION_COMMIT,
            TAG_RESPONSE_COMMIT,
            start.elapsed(),
        );

        SingleResponse::completed(commit_response)
    }

    fn run_genesis(
        &self,
        _request_options: RequestOptions,
        genesis_config: ChainSpec_GenesisConfig,
    ) -> SingleResponse<GenesisResponse> {
        let start = Instant::now();
        let correlation_id = CorrelationId::new();

        let genesis_config: GenesisConfig = match genesis_config.try_into() {
            Ok(genesis_config) => genesis_config,
            Err(error) => {
                let err_msg = error.to_string();
                warn!("{}", err_msg);

                let mut genesis_response = GenesisResponse::new();
                genesis_response.mut_failed_deploy().set_message(err_msg);
                return SingleResponse::completed(genesis_response);
            }
        };

        let genesis_response = match self.commit_genesis(correlation_id, genesis_config) {
            Ok(GenesisResult::Success {
                post_state_hash,
                effect,
            }) => {
                let success_message = format!("run_genesis successful: {}", post_state_hash);
                info!("{}", success_message);

                let mut genesis_response = GenesisResponse::new();
                let genesis_result = genesis_response.mut_success();
                genesis_result.set_poststate_hash(post_state_hash.to_vec());
                genesis_result.set_effect(effect.into());
                genesis_response
            }
            Ok(genesis_result) => {
                let err_msg = genesis_result.to_string();
                warn!("{}", err_msg);

                let mut genesis_response = GenesisResponse::new();
                genesis_response.mut_failed_deploy().set_message(err_msg);
                genesis_response
            }
            Err(err) => {
                let err_msg = err.to_string();
                warn!("{}", err_msg);

                let mut genesis_response = GenesisResponse::new();
                genesis_response.mut_failed_deploy().set_message(err_msg);
                genesis_response
            }
        };

        log_duration(
            correlation_id,
            METRIC_DURATION_GENESIS,
            TAG_RESPONSE_GENESIS,
            start.elapsed(),
        );

        SingleResponse::completed(genesis_response)
    }

    fn upgrade(
        &self,
        _request_options: RequestOptions,
        upgrade_request: UpgradeRequest,
    ) -> SingleResponse<UpgradeResponse> {
        let start = Instant::now();
        let correlation_id = CorrelationId::new();

        let upgrade_config: UpgradeConfig = match upgrade_request.try_into() {
            Ok(upgrade_config) => upgrade_config,
            Err(error) => {
                let err_msg = error.to_string();
                warn!("{}", err_msg);

                let mut upgrade_response = UpgradeResponse::new();
                upgrade_response.mut_failed_deploy().set_message(err_msg);

                log_duration(
                    correlation_id,
                    METRIC_DURATION_UPGRADE,
                    TAG_RESPONSE_UPGRADE,
                    start.elapsed(),
                );

                return SingleResponse::completed(upgrade_response);
            }
        };

        let upgrade_response = match self.commit_upgrade(correlation_id, upgrade_config) {
            Ok(UpgradeResult::Success {
                post_state_hash,
                effect,
            }) => {
                info!("upgrade successful: {}", post_state_hash);
                let mut ret = UpgradeResponse::new();
                let upgrade_result = ret.mut_success();
                upgrade_result.set_post_state_hash(post_state_hash.to_vec());
                upgrade_result.set_effect(effect.into());
                ret
            }
            Ok(upgrade_result) => {
                let err_msg = upgrade_result.to_string();
                warn!("{}", err_msg);

                let mut ret = UpgradeResponse::new();
                ret.mut_failed_deploy().set_message(err_msg);
                ret
            }
            Err(err) => {
                let err_msg = err.to_string();
                warn!("{}", err_msg);

                let mut ret = UpgradeResponse::new();
                ret.mut_failed_deploy().set_message(err_msg);
                ret
            }
        };

        log_duration(
            correlation_id,
            METRIC_DURATION_UPGRADE,
            TAG_RESPONSE_UPGRADE,
            start.elapsed(),
        );

        SingleResponse::completed(upgrade_response)
    }

    fn bid_state(
        &self,
        _request_options: RequestOptions,
        _bid_state_request: BidStateRequest,
    ) -> SingleResponse<BidStateResponse> {
        SingleResponse::err(GrpcError::Panic(UNIMPLEMENTED.to_string()))
    }

    fn distribute_rewards(
        &self,
        _request_options: RequestOptions,
        _distribute_rewards_request: DistributeRewardsRequest,
    ) -> SingleResponse<DistributeRewardsResponse> {
        SingleResponse::err(GrpcError::Panic(UNIMPLEMENTED.to_string()))
    }

    fn slash(
        &self,
        _request_options: RequestOptions,
        _slash_request: SlashRequest,
    ) -> SingleResponse<SlashResponse> {
        SingleResponse::err(GrpcError::Panic(UNIMPLEMENTED.to_string()))
    }

    fn unbond_payout(
        &self,
        _request_options: RequestOptions,
        _unbond_payout_request: UnbondPayoutRequest,
    ) -> SingleResponse<UnbondPayoutResponse> {
        SingleResponse::err(GrpcError::Panic(UNIMPLEMENTED.to_string()))
    }
}

// Helper method which returns single DeployResult that is set to be a
// WasmError.
pub fn new<E: ExecutionEngineService + Sync + Send + 'static>(
    socket: &str,
    thread_count: usize,
    e: E,
) -> ServerBuilder {
    let socket_path = std::path::Path::new(socket);

    if let Err(e) = std::fs::remove_file(socket_path) {
        if e.kind() != ErrorKind::NotFound {
            panic!("failed to remove old socket file: {:?}", e);
        }
    }

    let mut server = ServerBuilder::new_plain();
    server.http.set_unix_addr(socket.to_owned()).unwrap();
    server.http.set_cpu_pool_threads(thread_count);
    server.add_service(ExecutionEngineServiceServer::new_service_def(e));
    server
}
