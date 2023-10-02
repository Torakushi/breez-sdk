use std::sync::{Arc, RwLock};
use std::collections::HashMap;

use anyhow::{anyhow, Result};
use log::{Level, LevelFilter, Metadata, Record, Log};
use once_cell::sync::{Lazy, OnceCell};
use tracing::field;
use tracing::field::Visit;
use tracing::Subscriber;
use tracing_log::LogTracer;
use tracing_subscriber::prelude::*;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::Layer;

use breez_sdk_core::{
    error::*, mnemonic_to_seed as sdk_mnemonic_to_seed, parse as sdk_parse_input,
    parse_invoice as sdk_parse_invoice, AesSuccessActionDataDecrypted, BackupFailedData,
    BackupStatus, BitcoinAddressData, BreezEvent, BreezServices, BuyBitcoinProvider,
    BuyBitcoinRequest, BuyBitcoinResponse, ChannelState, CheckMessageRequest, CheckMessageResponse,
    ClosedChannelPaymentDetails, Config, CurrencyInfo, EnvironmentType, EventListener,
    FeeratePreset, FiatCurrency, GreenlightCredentials, GreenlightNodeConfig, InputType,
    InvoicePaidDetails, LNInvoice, ListPaymentsRequest, LnPaymentDetails, LnUrlAuthRequestData,
    LnUrlCallbackStatus, LnUrlErrorData, LnUrlPayRequestData, LnUrlPayResult,
    LnUrlWithdrawRequestData, LnUrlWithdrawResult, LnUrlWithdrawSuccessData, LocaleOverrides,
    LocalizedName, LogEntry, LogLevel, LogMessage, LogStream, Logger, LspInformation,
    MessageSuccessActionData, MetadataItem, Network, NodeConfig, NodeState, OpenChannelFeeRequest,
    OpenChannelFeeResponse, OpeningFeeParams, OpeningFeeParamsMenu, Payment, PaymentDetails,
    PaymentFailedData, PaymentStatus, PaymentType, PaymentTypeFilter, Rate, ReceiveOnchainRequest,
    ReceivePaymentRequest, ReceivePaymentResponse, RecommendedFees, ReverseSwapFeesRequest,
    ReverseSwapInfo, ReverseSwapPairInfo, ReverseSwapStatus, RouteHint, RouteHintHop,
    SignMessageRequest, SignMessageResponse, StaticBackupRequest, StaticBackupResponse,
    SuccessActionProcessed, SwapInfo, SwapStatus, SweepRequest, SweepResponse, Symbol,
    UnspentTransactionOutput, UrlSuccessActionData,
};
static RT: Lazy<tokio::runtime::Runtime> = Lazy::new(|| tokio::runtime::Runtime::new().unwrap());
static LOG_INIT: OnceCell<bool> = OnceCell::new();

lazy_static::lazy_static! {
    static ref LOCAL_LOGGERS: Arc<RwLock<HashMap<String, Arc<Box<dyn Logger>>>>> = Arc::new(RwLock::new(HashMap::new()));
}

struct BindingLogger {
    log_stream: Box<dyn LogStream>,
}

impl BindingLogger {
    fn init(log_stream: Box<dyn LogStream>) {
        let binding_logger = BindingLogger {log_stream:  log_stream };
        log::set_boxed_logger(Box::new(binding_logger)).unwrap();
        log::set_max_level(LevelFilter::Trace);
    }
}

pub struct CustomLayer {
    global_logger: Box<dyn log::Log>,
    local_loggers: Arc<RwLock<HashMap<String, Arc<Box<(dyn Logger)>>>>>,
}

impl CustomLayer {
    fn new() -> CustomLayer {
        CustomLayer {
            global_logger: Box::new(log::logger()),
            local_loggers: LOCAL_LOGGERS.clone(),
        }
    }

    fn set_global_logger(&mut self, global_logger: Box<dyn log::Log>){
        self.global_logger = global_logger
    }
}

#[derive(Debug)]
struct CustomFieldStorage(HashMap<String, String>);
impl<S> Layer<S> for CustomLayer
where
    S: Subscriber,
    S: for<'lookup> LookupSpan<'lookup>,
{
    fn on_new_span(
        &self,
        attrs: &tracing::span::Attributes<'_>,
        id: &tracing::span::Id,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        println!("{:?}", id);
        let span = ctx.span(id).unwrap();
        let mut fields = HashMap::new();
        let mut visitor = CustomVisitor(&mut fields);
        attrs.record(&mut visitor);
        let storage = CustomFieldStorage(fields);
        let mut extensions = span.extensions_mut();
        extensions.insert(storage);
    }

    fn on_event(&self, event: &tracing::Event<'_>, ctx: tracing_subscriber::layer::Context<'_, S>) {
        let mut h = HashMap::new();
        let visitor: &mut dyn field::Visit = &mut CustomVisitor(&mut h);
        event.record(visitor);

        // If we have a span context, then it means that the log was emited from a special
        // SDK instances. We map to the right logger.
        let mut seed: Option<String> = None;
        if let Some(scope) = ctx.event_scope(event) {
            for span in scope.from_root() {
                if let Some(extension) = span.extensions().get::<CustomFieldStorage>() {
                    seed = extension.0.get("seed").cloned();
                    break;
                }
            }
        }

        let mut h = HashMap::new();
        let visitor: &mut dyn field::Visit = &mut CustomVisitor(&mut h);
        event.record(visitor);



        if let Some(seed) = seed {
            // We got a seed. It should belong to one of our local logger
            if let Some(logger) = self.local_loggers.read().unwrap().get(&seed){
                logger.log(convert_to_log_message(&h));
                return;
            }
        }

        let empty_string: String = "".to_string();

        let metadata = Metadata::builder()
            .target(h.get("log.target").unwrap_or(&empty_string))
            .level(Level::Info)
            .build();


        let line = h.get("log.line").and_then(|s| s.parse().ok());
        let file: &String = h.get("log.file").unwrap_or(&empty_string);
        let module_path = h.get("log.module_path").unwrap_or(&empty_string);

        self.global_logger.log(&Record::builder()
        .metadata(metadata)
        .args(format_args!("{}", h.get("message").unwrap_or(&empty_string)))
        .line(line)
        .file(Some(file))
        .module_path(Some(module_path))
        .build());



        }

        }

struct CustomVisitor<'a>(&'a mut HashMap<String, String>);

impl<'a> Visit for CustomVisitor<'a> {
    fn record_f64(&mut self, field: &tracing::field::Field, value: f64) {
        self.0.insert(field.name().to_string(), value.to_string());
    }

    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        self.0.insert(field.name().to_string(), value.to_string());
    }

    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        self.0.insert(field.name().to_string(), value.to_string());
    }

    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        self.0.insert(field.name().to_string(), value.to_string());
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.0.insert(field.name().to_string(), value.to_string());
    }

    fn record_error(
        &mut self,
        field: &tracing::field::Field,
        value: &(dyn std::error::Error + 'static),
    ) {
        self.0.insert(field.name().to_string(), value.to_string());
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.0
            .insert(field.name().to_string(), format!("{:?}", value));
    }
}

fn convert_to_log_message(data: &HashMap<String, String>) -> LogMessage {
    LogMessage {
        level: LogLevel::Info,
        message: data
            .get("message")
            .map(|s| s.to_string())
            .unwrap_or_default(),
        module_path: data
            .get("log.module_path")
            .map(|s| s.to_string())
            .unwrap_or_default(),
        file: data
            .get("log.file")
            .map(|s| s.to_string())
            .unwrap_or_default(),
        line: data
            .get("log.line")
            .and_then(|s| s.parse().ok())
            .unwrap_or_default(),
    }
}

impl log::Log for BindingLogger {
    fn enabled(&self, m: &Metadata) -> bool {
        // ignore the internal uniffi log to prevent infinite loop.
        return m.level() <= Level::Trace && *m.target() != *"breez_sdk_bindings::uniffi_binding";
    }

    fn log(&self, record: &Record) {
        self.log_stream.log(LogEntry {
            line: record.args().to_string(),
            level: record.level().as_str().to_string(),
        });
    }
    fn flush(&self) {}
}

/// Create a new SDK config with default values
pub fn default_config(
    env_type: EnvironmentType,
    api_key: String,
    node_config: NodeConfig,
) -> Config {
    BreezServices::default_config(env_type, api_key, node_config)
}

/// Get the static backup data from the peristent storage.
/// This data enables the user to recover the node in an external core ligntning node.
/// See here for instructions on how to recover using this data: https://docs.corelightning.org/docs/backup-and-recovery#backing-up-using-static-channel-backup
pub fn static_backup(request: StaticBackupRequest) -> SdkResult<StaticBackupResponse> {
    BreezServices::static_backup(request)
}

/// connect initializes the SDK services, schedule the node to run in the cloud and
/// run the signer. This must be called in order to start communicating with the node.
///
/// In addition, it also initializes SDK logging. If the log stream was already set using [`set_log_stream`]
/// when this is called, log statements are sent to the log stream.
///
/// # Arguments
///
/// * `config` - The sdk configuration
/// * `seed` - The node private key
/// * `event_listener` - Listener to SDK events
/// * `node_logger` - Local logger for the node
/// * `log_file_path` - an optional file path in which to write node logs
///
    pub fn connect(
        config: Config,
        seed: Vec<u8>,
        event_listener: Box<dyn EventListener>,
        node_logger: Box<dyn Logger>,
        log_file_path: Option<String>,
    ) -> SdkResult<Arc<BlockingBreezServices>> {
        let node_logger = Arc::new(node_logger);


        rt().block_on(async move {
            add_local_logger(hex::encode(seed.clone()), node_logger.clone() );

            let breez_services =
                BreezServices::connect(config, seed, event_listener, node_logger , log_file_path)
                    .await?;

            Ok(Arc::new(BlockingBreezServices { breez_services }))
        })
}

struct TestToRemoveLogger;

impl Logger for TestToRemoveLogger{
    fn log(&self, log_message: LogMessage) {
        println!("My_fake_second_logger: {}", log_message.message)
    }
}

fn add_local_logger(seed: String, logger: Arc<Box<dyn Logger>>) {
    // Acquire a write lock on LOCAL_LOGGERS and insert the logger.
    // TODO: check if seed already exists
    LOCAL_LOGGERS.write().unwrap().insert(seed.clone(), logger);
    println!("logger added {}", seed.clone());
}

pub struct NOOO;
impl LogStream for NOOO {
    fn log(&self, l: LogEntry){
        println!("{}", l.line)
    }
}

impl Logger for NOOO {
    fn log(&self, l: LogMessage){
        println!("NOOO, local_node_logger: {}", l.message)
    }
}

/// If used, this must be called before `connect`
pub fn set_log_stream(log_stream: Box<dyn LogStream>) -> Result<()> {
    LOG_INIT
        .set(true)
        .map_err(|_| anyhow!("log stream already created"))?;


    let mut layer = CustomLayer::new();
    layer.set_global_logger(Box::new(BindingLogger{log_stream}));

    let sub = tracing_subscriber::registry().with(layer);

    tracing::subscriber::set_global_default(sub).unwrap();
    LogTracer::init().unwrap();

    Ok(())
}

pub struct BlockingBreezServices {
    breez_services: Arc<BreezServices>,
}

impl BlockingBreezServices {
    pub fn disconnect(&self) -> Result<()> {
        rt().block_on(self.breez_services.disconnect())
    }

    pub fn send_payment(&self, bolt11: String, amount_sats: Option<u64>) -> SdkResult<Payment> {
        rt().block_on(self.breez_services.send_payment(bolt11, amount_sats))
    }

    pub fn send_spontaneous_payment(
        &self,
        node_id: String,
        amount_sats: u64,
    ) -> SdkResult<Payment> {
        rt().block_on(
            self.breez_services
                .send_spontaneous_payment(node_id, amount_sats),
        )
    }

    pub fn receive_payment(
        &self,
        req_data: ReceivePaymentRequest,
    ) -> SdkResult<ReceivePaymentResponse> {
        rt().block_on(self.breez_services.receive_payment(req_data))
    }

    pub fn node_info(&self) -> SdkResult<NodeState> {
        self.breez_services.node_info()
    }

    pub fn sign_message(&self, request: SignMessageRequest) -> SdkResult<SignMessageResponse> {
        rt().block_on(self.breez_services.sign_message(request))
            .map_err(|e| e.into())
    }

    pub fn check_message(&self, request: CheckMessageRequest) -> SdkResult<CheckMessageResponse> {
        rt().block_on(self.breez_services.check_message(request))
            .map_err(|e| e.into())
    }

    pub fn backup_status(&self) -> SdkResult<BackupStatus> {
        self.breez_services.backup_status().map_err(|e| e.into())
    }

    pub fn backup(&self) -> SdkResult<()> {
        rt().block_on(self.breez_services.backup())
            .map_err(|e| e.into())
    }

    pub fn list_payments(&self, request: ListPaymentsRequest) -> SdkResult<Vec<Payment>> {
        rt().block_on(self.breez_services.list_payments(request))
    }

    pub fn payment_by_hash(&self, hash: String) -> SdkResult<Option<Payment>> {
        rt().block_on(self.breez_services.payment_by_hash(hash))
            .map_err(|e| e.into())
    }

    pub fn pay_lnurl(
        &self,
        req_data: LnUrlPayRequestData,
        amount_sats: u64,
        comment: Option<String>,
    ) -> SdkResult<LnUrlPayResult> {
        rt().block_on(
            self.breez_services
                .lnurl_pay(amount_sats, comment, req_data),
        )
        .map_err(|e| e.into())
    }

    pub fn withdraw_lnurl(
        &self,
        req_data: LnUrlWithdrawRequestData,
        amount_sats: u64,
        description: Option<String>,
    ) -> SdkResult<LnUrlWithdrawResult> {
        rt().block_on(
            self.breez_services
                .lnurl_withdraw(req_data, amount_sats, description),
        )
        .map_err(|e| e.into())
    }

    pub fn lnurl_auth(&self, req_data: LnUrlAuthRequestData) -> SdkResult<LnUrlCallbackStatus> {
        rt().block_on(self.breez_services.lnurl_auth(req_data))
            .map_err(|e| e.into())
    }

    pub fn sweep(&self, request: SweepRequest) -> SdkResult<SweepResponse> {
        rt().block_on(self.breez_services.sweep(request))
            .map_err(|e| e.into())
    }

    pub fn fetch_fiat_rates(&self) -> SdkResult<Vec<Rate>> {
        rt().block_on(self.breez_services.fetch_fiat_rates())
            .map_err(|e| e.into())
    }

    pub fn list_fiat_currencies(&self) -> SdkResult<Vec<FiatCurrency>> {
        rt().block_on(self.breez_services.list_fiat_currencies())
            .map_err(|e| e.into())
    }

    pub fn list_lsps(&self) -> SdkResult<Vec<LspInformation>> {
        rt().block_on(self.breez_services.list_lsps())
    }

    pub fn connect_lsp(&self, lsp_id: String) -> SdkResult<()> {
        rt().block_on(self.breez_services.connect_lsp(lsp_id))
    }

    pub fn fetch_lsp_info(&self, lsp_id: String) -> SdkResult<Option<LspInformation>> {
        rt().block_on(self.breez_services.fetch_lsp_info(lsp_id))
            .map_err(|e| e.into())
    }

    pub fn lsp_id(&self) -> SdkResult<Option<String>> {
        rt().block_on(self.breez_services.lsp_id())
    }

    pub fn lsp_info(&self) -> SdkResult<LspInformation> {
        rt().block_on(self.breez_services.lsp_info())
            .map_err(|e: anyhow::Error| e.into())
    }

    pub fn open_channel_fee(
        &self,
        req: OpenChannelFeeRequest,
    ) -> SdkResult<OpenChannelFeeResponse> {
        rt().block_on(self.breez_services.open_channel_fee(req))
    }

    pub fn close_lsp_channels(&self) -> SdkResult<()> {
        rt().block_on(async {
            _ = self.breez_services.close_lsp_channels().await?;
            Ok(())
        })
        .map_err(|e: anyhow::Error| e.into())
    }

    /// Onchain receive swap API
    pub fn receive_onchain(&self, req: ReceiveOnchainRequest) -> SdkResult<SwapInfo> {
        rt().block_on(self.breez_services.receive_onchain(req))
            .map_err(|e| e.into())
    }

    /// Onchain receive swap API
    pub fn in_progress_swap(&self) -> SdkResult<Option<SwapInfo>> {
        rt().block_on(self.breez_services.in_progress_swap())
            .map_err(|e| e.into())
    }

    /// list non-completed expired swaps that should be refunded by calling [BreezServices::refund]
    pub fn list_refundables(&self) -> SdkResult<Vec<SwapInfo>> {
        rt().block_on(self.breez_services.list_refundables())
            .map_err(|e| e.into())
    }

    // construct and broadcast a refund transaction for a faile/expired swap
    pub fn refund(
        &self,
        swap_address: String,
        to_address: String,
        sat_per_vbyte: u32,
    ) -> SdkResult<String> {
        rt().block_on(
            self.breez_services
                .refund(swap_address, to_address, sat_per_vbyte),
        )
        .map_err(|e| e.into())
    }

    pub fn fetch_reverse_swap_fees(
        &self,
        req: ReverseSwapFeesRequest,
    ) -> SdkResult<ReverseSwapPairInfo> {
        rt().block_on(self.breez_services.fetch_reverse_swap_fees(req))
            .map_err(|e| e.into())
    }

    pub fn in_progress_reverse_swaps(&self) -> SdkResult<Vec<ReverseSwapInfo>> {
        rt().block_on(self.breez_services.in_progress_reverse_swaps())
            .map_err(|e| e.into())
    }

    pub fn send_onchain(
        &self,
        amount_sat: u64,
        onchain_recipient_address: String,
        pair_hash: String,
        sat_per_vbyte: u64,
    ) -> SdkResult<ReverseSwapInfo> {
        rt().block_on(self.breez_services.send_onchain(
            amount_sat,
            onchain_recipient_address,
            pair_hash,
            sat_per_vbyte,
        ))
        .map_err(|e| e.into())
    }

    pub fn execute_dev_command(&self, command: String) -> Result<String> {
        rt().block_on(self.breez_services.execute_dev_command(command))
    }

    pub fn sync(&self) -> SdkResult<()> {
        rt().block_on(self.breez_services.sync())
            .map_err(|e| e.into())
    }

    pub fn recommended_fees(&self) -> SdkResult<RecommendedFees> {
        rt().block_on(self.breez_services.recommended_fees())
            .map_err(|e| e.into())
    }

    pub fn buy_bitcoin(&self, req: BuyBitcoinRequest) -> SdkResult<BuyBitcoinResponse> {
        rt().block_on(self.breez_services.buy_bitcoin(req))
    }
}

pub fn parse_invoice(invoice: String) -> SdkResult<LNInvoice> {
    sdk_parse_invoice(&invoice).map_err(|e| e.into())
}

pub fn parse_input(s: String) -> SdkResult<InputType> {
    rt().block_on(sdk_parse_input(&s)).map_err(|e| e.into())
}

pub fn mnemonic_to_seed(phrase: String) -> SdkResult<Vec<u8>> {
    sdk_mnemonic_to_seed(phrase).map_err(|e| e.into())
}

fn rt() -> &'static tokio::runtime::Runtime {
    &RT
}

uniffi_macros::include_scaffolding!("breez_sdk");
