"""Builds comprehensive JSON catalogs for Oyenza Studio.
These can be hosted on Cloudflare Pages (e.g. /catalogs/crypto.json)
and cached locally in %LOCALAPPDATA%/Poni/catalogs/.
"""

import json
import os
import re

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CATALOGS_DIR = os.path.join(ROOT, "catalogs")
os.makedirs(CATALOGS_DIR, exist_ok=True)

# 1. Parse KNOWN_COINS from crates/poni-core/src/crypto.rs
crypto_file = os.path.join(ROOT, "crates", "poni-core", "src", "crypto.rs")
with open(crypto_file, "r", encoding="utf-8") as f:
    crypto_content = f.read()

coins = []
seen_coins = set()
for m in re.finditer(r'\(\s*"([^"]+)"\s*,\s*"([^"]+)"\s*\)', crypto_content):
    c, n = m.group(1), m.group(2)
    # Check if this belongs to KNOWN_COINS
    if c not in seen_coins and len(c) <= 10 and c.isupper():
        seen_coins.add(c)
        coins.append([c, n])

# 2. Add extra popular coins
extra_coins = [
    ["BLUR", "Blur"],
    ["ID", "SPACE ID"],
    ["EDU", "Open Campus"],
    ["MAGIC", "Magic"],
    ["HIGH", "Highstreet"],
    ["HOOK", "Hooked Protocol"],
    ["GMX", "GMX"],
    ["DYDX", "dYdX"],
    ["WLD", "Worldcoin"],
    ["ARKM", "Arkham"],
    ["MAV", "Maverick Protocol"],
    ["PENDLE", "Pendle"],
    ["CYBER", "CyberConnect"],
    ["SEI", "Sei Network"],
    ["TIA", "Celestia"],
    ["MEME", "Memecoin"],
    ["ORDI", "Ordinals"],
    ["BEAM", "Beam"],
    ["VANRY", "Vanar Chain"],
    ["1000SATS", "SATS Ordinals"],
    ["BONK", "Bonk"],
    ["ACE", "Fusionist"],
    ["NFP", "NFPrompt"],
    ["AI", "Sleepless AI"],
    ["XAI", "Xai"],
    ["MANTA", "Manta Network"],
    ["ALT", "AltLayer"],
    ["JUP", "Jupiter"],
    ["PYTH", "Pyth Network"],
    ["RONIN", "Ronin"],
    ["DYM", "Dymension"],
    ["PIXEL", "Pixels"],
    ["STRK", "Starknet"],
    ["PORTAL", "Portal"],
    ["PDA", "PlayDapp"],
    ["AXL", "Axelar"],
    ["METIS", "Metis"],
    ["AEVO", "Aevo"],
    ["BOME", "BOOK OF MEME"],
    ["ETHFI", "ether.fi"],
    ["ENA", "Ethena"],
    ["W", "Wormhole"],
    ["TNSR", "Tensor"],
    ["SAGA", "Saga"],
    ["TAO", "Bittensor"],
    ["OMNI", "Omni Network"],
    ["REZ", "Renzo"],
    ["BB", "BounceBit"],
    ["NOT", "Notcoin"],
    ["IO", "io.net"],
    ["ZK", "ZKsync"],
    ["LISTA", "Lista DAO"],
    ["ZRO", "LayerZero"],
    ["G", "Gravity"],
    ["BANANA", "Banana Gun"],
    ["RENDER", "Render"],
    ["TON", "Toncoin"],
    ["DOGS", "Dogs"],
    ["EURI", "Eurite"],
    ["CATI", "Catizen"],
    ["HMSTR", "Hamster Kombat"],
    ["EIGEN", "EigenLayer"],
    ["SCR", "Scroll"],
    ["BNSOL", "Binance Staked SOL"],
    ["COW", "CoW Protocol"],
    ["CETUS", "Cetus Protocol"],
    ["ACT", "Act I : The AI Prophecy"],
    ["PNUT", "Peanut the Squirrel"],
    ["USUAL", "Usual"],
    ["THE", "THENA"],
    ["MOVE", "Movement"],
    ["ME", "Magic Eden"],
    ["PENGU", "Pudgy Penguins"],
    ["BIO", "Bio Protocol"],
    ["TRUMP", "Official Trump"],
    ["MELANIA", "Melania Trump"],
    ["SWARMS", "Swarms"],
    ["VIRTUAL", "Virtuals Protocol"],
    ["AI16Z", "ai16z"],
    ["FARTCOIN", "Fartcoin"],
    ["GRIFFAIN", "Griffain"],
    ["AIXBT", "aixbt"],
    ["COOKIE", "Cookie DAO"]
]

for c, n in extra_coins:
    if c not in seen_coins:
        seen_coins.add(c)
        coins.append([c, n])

crypto_path = os.path.join(CATALOGS_DIR, "crypto.json")
with open(crypto_path, "w", encoding="utf-8") as f:
    json.dump(coins, f, indent=2)
print(f"Generated {crypto_path} ({len(coins)} coins)")

# 3. Parse KNOWN_US_STOCKS from crates/poni-core/src/us_stocks.rs
stocks_file = os.path.join(ROOT, "crates", "poni-core", "src", "us_stocks.rs")
with open(stocks_file, "r", encoding="utf-8") as f:
    stocks_content = f.read()

stocks = []
seen_stocks = set()
for m in re.finditer(r'\(\s*"([^"]+)"\s*,\s*"([^"]+)"\s*\)', stocks_content):
    c, n = m.group(1), m.group(2)
    if c not in seen_stocks and len(c) <= 6 and c.isupper():
        seen_stocks.add(c)
        stocks.append([c, n])

# Add more leading US stocks
extra_stocks = [
    ["VRT", "Vertiv Holdings"],
    ["GE", "GE Aerospace"],
    ["CEG", "Constellation Energy"],
    ["VST", "Vistra Corp."],
    ["TLN", "Talen Energy"],
    ["NRG", "NRG Energy"],
    ["ETN", "Eaton Corporation"],
    ["PH", "Parker-Hannifin"],
    ["EME", "EMCOR Group"],
    ["PWR", "Quanta Services"],
    ["URI", "United Rentals"],
    ["DE", "Deere & Company"],
    ["CAT", "Caterpillar Inc."],
    ["UNP", "Union Pacific"],
    ["CSX", "CSX Corporation"],
    ["FDX", "FedEx Corporation"],
    ["UPS", "United Parcel Service"],
    ["WM", "Waste Management"],
    ["RSG", "Republic Services"],
    ["CARR", "Carrier Global"],
    ["TT", "Trane Technologies"],
    ["LII", "Lennox International"],
    ["JCI", "Johnson Controls"],
    ["ITW", "Illinois Tool Works"],
    ["MMM", "3M Company"],
    ["HON", "Honeywell International"],
    ["RTX", "RTX Corporation"],
    ["LMT", "Lockheed Martin"],
    ["NOC", "Northrop Grumman"],
    ["GD", "General Dynamics"],
    ["BA", "Boeing Company"],
    ["TDG", "TransDigm Group"],
    ["HEI", "HEICO Corporation"],
    ["HWM", "Howmet Aerospace"],
    ["AXON", "Axon Enterprise"],
    ["LHX", "L3Harris Technologies"],
    ["KTOS", "Kratos Defense"],
    ["AVAV", "AeroVironment"],
    ["RKLB", "Rocket Lab USA"],
    ["LUNR", "Intuitive Machines"],
    ["ASTS", "AST SpaceMobile"],
    ["JOBY", "Joby Aviation"],
    ["ACHR", "Archer Aviation"],
    ["BLDE", "Blade Air Mobility"]
]

for s, n in extra_stocks:
    if s not in seen_stocks:
        seen_stocks.add(s)
        stocks.append([s, n])

us_stocks_path = os.path.join(CATALOGS_DIR, "us_stocks.json")
with open(us_stocks_path, "w", encoding="utf-8") as f:
    json.dump(stocks, f, indent=2)
print(f"Generated {us_stocks_path} ({len(stocks)} US stocks)")

# 4. FOREX
forex_items = [
    ["USD/IDR", "US Dollar"],
    ["GOLD", "Gold (Antam / g)"],
    ["SILVER", "Silver (Spot / g)"],
    ["OIL", "WTI Crude Oil (USD)"],
    ["SGD/IDR", "Singapore Dollar"],
    ["EUR/IDR", "Euro"],
    ["JPY/IDR", "Japanese Yen (100)"],
    ["SAR/IDR", "Saudi Riyal"],
    ["MYR/IDR", "Malaysian Ringgit"],
    ["CNY/IDR", "Chinese Yuan"],
    ["AUD/IDR", "Australian Dollar"],
    ["GBP/IDR", "British Pound"],
    ["CHF/IDR", "Swiss Franc"],
    ["CAD/IDR", "Canadian Dollar"],
    ["HKD/IDR", "Hong Kong Dollar"],
    ["NZD/IDR", "New Zealand Dollar"],
    ["KRW/IDR", "South Korean Won (100)"],
    ["THB/IDR", "Thai Baht"],
    ["INR/IDR", "Indian Rupee"],
    ["TWD/IDR", "Taiwan Dollar"],
    ["AED/IDR", "UAE Dirham"],
    ["PHP/IDR", "Philippine Peso"],
    ["PLATINUM", "Platinum Spot (per g)"],
    ["BRENT", "Brent Crude Oil (USD)"],
    ["NATURAL_GAS", "Natural Gas (USD)"],
    ["COPPER", "Copper Spot (USD/lb)"],
    ["PALLADIUM", "Palladium Spot (per g)"],
    ["VND/IDR", "Vietnamese Dong (100)"],
    ["BRL/IDR", "Brazilian Real"],
    ["RUB/IDR", "Russian Ruble"],
    ["ZAR/IDR", "South African Rand"],
    ["TRY/IDR", "Turkish Lira"],
    ["MXN/IDR", "Mexican Peso"],
    ["SEK/IDR", "Swedish Krona"],
    ["NOK/IDR", "Norwegian Krone"],
    ["DKK/IDR", "Danish Krone"],
    ["PLN/IDR", "Polish Zloty"],
    ["HUF/IDR", "Hungarian Forint"],
    ["CZK/IDR", "Czech Koruna"],
    ["ILS/IDR", "Israeli Shekel"],
    ["CLP/IDR", "Chilean Peso"],
    ["EGP/IDR", "Egyptian Pound"],
    ["PKR/IDR", "Pakistani Rupee"],
    ["KWD/IDR", "Kuwaiti Dinar"],
    ["QAR/IDR", "Qatari Riyal"],
    ["BHD/IDR", "Bahraini Dinar"],
    ["OMR/IDR", "Omani Rial"],
    ["EUR/USD", "Euro / US Dollar"],
    ["GBP/USD", "British Pound / US Dollar"],
    ["USD/JPY", "US Dollar / Japanese Yen"],
    ["AUD/USD", "Australian Dollar / US Dollar"],
    ["USD/CHF", "US Dollar / Swiss Franc"],
    ["USD/CAD", "US Dollar / Canadian Dollar"],
    ["NZD/USD", "NZ Dollar / US Dollar"],
    ["ALUMINUM", "Aluminum Futures (USD/t)"],
    ["NICKEL", "Nickel Futures (USD/t)"],
    ["ZINC", "Zinc Futures (USD/t)"],
    ["LEAD", "Lead Futures (USD/t)"],
    ["TIN", "Tin Futures (USD/t)"],
    ["URANIUM", "Uranium Futures (USD/lb)"],
    ["CORN", "Corn Futures (USD/bu)"],
    ["WHEAT", "Wheat Futures (USD/bu)"],
    ["SOYBEANS", "Soybean Futures (USD/bu)"],
    ["COFFEE", "Coffee Futures (USD/lb)"],
    ["SUGAR", "Sugar Futures (USD/lb)"],
    ["COCOA", "Cocoa Futures (USD/t)"],
    ["COTTON", "Cotton Futures (USD/lb)"],
    ["PALM_OIL", "CPO Crude Palm Oil (MYR/t)"]
]

forex_path = os.path.join(CATALOGS_DIR, "forex.json")
with open(forex_path, "w", encoding="utf-8") as f:
    json.dump(forex_items, f, indent=2)
print(f"Generated {forex_path} ({len(forex_items)} items)")
