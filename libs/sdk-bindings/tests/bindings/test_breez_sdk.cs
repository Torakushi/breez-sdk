
using breez.breez_sdk;

try
{
 var seed = BreezSdkMethods.MnemonicToSeed("cruise clever syrup coil cute execute laundry general cover prevent law sheriff");
 var seed2 = BreezSdkMethods.MnemonicToSeed("park remain person kitchen mule spell knee armed position rail grid ankle");

 BreezSdkMethods.SetLogStream(new LogStreamListener());
 var config = BreezSdkMethods.DefaultConfig(EnvironmentType.STAGING, "code", new NodeConfig.Greenlight(new GreenlightNodeConfig(null, "")));

 BlockingBreezServices sdkServices = BreezSdkMethods.Connect(config, seed, new SDKListener(), new SDKNodeLogger(seed), null);
 BlockingBreezServices sdkServices2 = BreezSdkMethods.Connect(config, seed2, new SDKListener(), new SDKNodeLogger(seed2), null);

 NodeState? nodeInfo = sdkServices.NodeInfo();
 Console.WriteLine(nodeInfo!.id);
}
catch (Exception e)
{
 Console.WriteLine(e.Message);
}

class SDKListener : EventListener
{
 public void OnEvent(BreezEvent e)
 {
  Console.WriteLine("received event " + e);
 }
}

class SDKNodeLogger : Logger
{
 private string s; // Add a private field for the string

 public SDKNodeLogger(string s)
 {
    this.s = s;
 }
 public void Log(LogMessage l)
 {
  Console.WriteLine($"local_node_logger {s}: {l.message}");
 }
}

class LogStreamListener : LogStream
{
 public void Log(LogEntry l)
 {
  Console.WriteLine(l.line);
 }
}
