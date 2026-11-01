export async function onRequest(context) {
  return new Response(JSON.stringify({
    status: "ok",
    service: "DollarKilat Market Hub",
    domain: "api.dollarkilat.xyz",
    time: new Date().toISOString(),
    version: "1.0.0",
    endpoints: [
      "/v1/us-stocks/quotes?symbols=AAPL,NVDA,TSLA",
      "/v1/us-stocks/search?q=apple",
      "/v1/us-stocks/history?symbol=AAPL&range=1y",
      "/v1/forex/board?symbols=USD/IDR,GOLD,OIL",
      "/v1/forex/search?q=euro",
      "/v1/forex/history?symbol=USDIDR=X&range=1y",
      "/v1/crypto/board?symbols=BTC,ETH,SOL",
      "/v1/crypto/search?q=btc"
    ]
  }), {
    headers: {
      "Access-Control-Allow-Origin": "*",
      "Content-Type": "application/json; charset=utf-8",
      "Cache-Control": "public, max-age=10"
    }
  });
}
