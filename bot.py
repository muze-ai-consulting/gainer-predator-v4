import os
import logging
import asyncio
import math
from dotenv import load_dotenv
from telegram import Update
from telegram.ext import ApplicationBuilder, ContextTypes, CommandHandler
from binance.client import Client
from binance.exceptions import BinanceAPIException
from telethon import TelegramClient, events
from google import genai
from PIL import Image
from telegram import Update, BotCommand
import io
import json
from datetime import datetime, timezone, timedelta
from ddgs import DDGS

# Startup timestamp to ignore old messages (with 10-second safety buffer)
STARTUP_TIME = datetime.now(timezone.utc) + timedelta(seconds=10)

# Deduplication cache to prevent re-processing ghost/re-sent messages
PROCESSED_MESSAGES = set()

# Load environment variables
load_dotenv()

# Logger setup
logging.basicConfig(
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s',
    level=logging.INFO
)
logger = logging.getLogger(__name__)

DAILY_LOG_FILE = 'daily_actions.log'

def log_daily_action(action_msg):
    """Logs clean, high-level actions for future AI learning."""
    timestamp = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
    log_entry = f"[{timestamp}] {action_msg}\n"
    try:
        with open(DAILY_LOG_FILE, 'a') as f:
            f.write(log_entry)
        logger.info(f"📜 Action logged: {action_msg}")
    except Exception as e:
        logger.error(f"Error logging daily action: {e}")

# Configure Gemini
client_genai = genai.Client(api_key=os.getenv('GEMINI_API_KEY'))
model_id = "gemini-3-flash-preview"

# Target Channel for monitored signals
TARGET_CHANNEL_ID = -4661817763

# Global deduplication
PROCESSED_MESSAGES_FILE = 'processed_signals.json'

def load_processed_messages():
    if os.path.exists(PROCESSED_MESSAGES_FILE):
        with open(PROCESSED_MESSAGES_FILE, 'r') as f:
            try:
                return set(json.load(f))
            except:
                return set()
    return set()

def save_processed_messages(messages):
    with open(PROCESSED_MESSAGES_FILE, 'w') as f:
        json.dump(list(messages), f)

PROCESSED_MESSAGES = load_processed_messages()

SETTINGS_FILE = 'settings.json'

def load_settings():
    if os.path.exists(SETTINGS_FILE):
        with open(SETTINGS_FILE, 'r') as f:
            return json.load(f)
    return {"risk_percent": 0.10, "default_leverage": 10, "margin_type": "ISOLATED", "owner_chat_id": None}

def save_settings(settings):
    with open(SETTINGS_FILE, 'w') as f:
        json.dump(settings, f, indent=4)

async def start(update: Update, context: ContextTypes.DEFAULT_TYPE):
    s = load_settings()
    s['owner_chat_id'] = update.effective_chat.id
    save_settings(s)
    
    msg = (
        "🚀 **Tarzanes Trader Bot - Online**\n\n"
        "He guardado tu ID para enviarte notificaciones. Ahora monitorizo señales con **Gemini 3 Flash**.\n\n"
        "**Comandos Disponibles:**\n"
        "• /balance - Ver saldo en Binance\n"
        "• /settings - Ver/Cambiar configuración\n\n"
        "**¿Cómo funciona?**\n"
        "1. Envía `#btc` en el canal habilitado.\n"
        "2. Investigaré en la web (Reddit, News, TA).\n"
        "3. Gemini 3 decidirá y ejecutará con seguridad."
    )
    await update.message.reply_text(msg, parse_mode='Markdown')

async def balance(update: Update, context: ContextTypes.DEFAULT_TYPE):
    testnet = os.getenv('USE_TESTNET', 'True') == 'True'
    try:
        client = get_futures_client()
        res = client.futures_account_balance()
        usdt_balance = next((item for item in res if item["asset"] == "USDT"), None)
        if usdt_balance:
            await update.message.reply_text(f"💰 Balance USDT: {usdt_balance['balance']}")
    except Exception as e:
        await update.message.reply_text(f"Error Binance: {str(e)}")

async def get_settings_handler(update: Update, context: ContextTypes.DEFAULT_TYPE):
    s = load_settings()
    msg = (
        "⚙️ **Configuración Actual:**\n"
        f"• Riesgo por trade: {int(s['risk_percent']*100)}%\n"
        f"• Apalancamiento: {s['default_leverage']}x\n"
        f"• Margen: {s['margin_type']}\n\n"
        "Comandos para cambiar:\n"
        "/set_risk <val> (ej: 10 para 10%)\n"
        "/set_leverage <val> (ej: 5)\n"
        "/set_margin <ISOLATED/CROSSED>"
    )
    await update.message.reply_text(msg, parse_mode='Markdown')

async def set_risk(update: Update, context: ContextTypes.DEFAULT_TYPE):
    try:
        val = float(context.args[0]) / 100
        s = load_settings()
        s['risk_percent'] = val
        save_settings(s)
        await update.message.reply_text(f"✅ Riesgo actualizado a {int(val*100)}%")
    except:
        await update.message.reply_text("Uso: /set_risk 10")

async def set_leverage(update: Update, context: ContextTypes.DEFAULT_TYPE):
    try:
        val = int(context.args[0])
        s = load_settings()
        s['default_leverage'] = val
        save_settings(s)
        await update.message.reply_text(f"✅ Apalancamiento actualizado a {val}x")
    except:
        await update.message.reply_text("Uso: /set_leverage 5")

async def set_margin(update: Update, context: ContextTypes.DEFAULT_TYPE):
    try:
        val = context.args[0].upper()
        if val not in ['ISOLATED', 'CROSSED']:
            raise ValueError()
        s = load_settings()
        s['margin_type'] = val
        save_settings(s)
        await update.message.reply_text(f"✅ Margen actualizado a {val}")
    except:
        await update.message.reply_text("Uso: /set_margin ISOLATED o /set_margin CROSSED")

async def post_init(application):
    """Registers commands to show in the Telegram Menu button."""
    commands = [
        BotCommand("start", "Iniciar el bot"),
        BotCommand("balance", "Ver balance de Binance"),
        BotCommand("settings", "Ver configuración de trading"),
        BotCommand("set_risk", "Ej: /set_risk 10 (para 10%)"),
        BotCommand("set_leverage", "Ej: /set_leverage 20"),
        BotCommand("set_margin", "ISOLATED o CROSSED"),
    ]
    await application.bot.set_my_commands(commands)
    # Also set for the owner specifically to ensure it shows up
    s = load_settings()
    if s.get('owner_chat_id'):
        await application.bot.set_my_commands(commands, scope={"type": "chat", "chat_id": s['owner_chat_id']})
    logger.info("Bot commands registered in Telegram Menu.")

# -----------------------------------------------------------------
# 2. BINANCE UTILITIES & TRADING
# -----------------------------------------------------------------

def get_futures_client():
    api_key = os.getenv('BINANCE_API_KEY', '').strip()
    api_secret = os.getenv('BINANCE_API_SECRET', '').strip()
    testnet = os.getenv('USE_TESTNET', 'True') == 'True'
    return Client(api_key, api_secret, testnet=testnet)

def check_binance_connection():
    """Verifies API connectivity at startup."""
    try:
        client = get_futures_client()
        # Test connection with a simple call
        client.futures_account_balance()
        logger.info("✅ Conexión con Binance Futures establecida correctamente.")
        return True
    except Exception as e:
        logger.error(f"❌ Error de conexión con Binance: {e}")
        return False

def round_step_size(quantity, step_size):
    """Rounds a quantity to the nearest step size precision."""
    import math
    precision = int(round(-math.log(step_size, 10), 0))
    return float(round(quantity, precision))

async def execute_trade(signal, rationale_msg="", telegram_context=None, reply_to_id=None, chat_id=None):
    """Parses signal and executes orders on Binance Futures."""
    try:
        # 1. Setup Client & Settings
        client = get_futures_client()
        s = load_settings()
        
        symbol = signal.get('symbol', '').upper().replace('/', '')
        if not symbol.endswith('USDT'):
            symbol += 'USDT'
        
        side = signal.get('side', '').upper()
        
        # 2. Leverage Safety Limits
        leverage = int(signal.get('leverage', s['default_leverage']))
        if symbol in ['BTCUSDT', 'ETHUSDT']:
            leverage = min(leverage, 10)
        else:
            leverage = min(leverage, 5)
        
        # --- DYNAMIC RISK CALCULATION ---
        base_risk = s.get('risk_percent', 0.1)
        # AI Risk Bonus (from signal parsing)
        ai_bonus = float(signal.get('risk_bonus', 0.0))
        total_risk = min(base_risk + ai_bonus, 0.8) # Cap at 80% for safety
        
        logger.info(f"Executing trade for {symbol} {side} x{leverage} (Total Risk: {total_risk:.2%})")
        
        # 2. Get Price & Balance first (needed for notification)
        mark_price = float(client.futures_symbol_ticker(symbol=symbol)['price'])
        
        # Feedback to user
        if telegram_context and s.get('owner_chat_id'):
            risk_info = f"• **Riesgo:** {total_risk:.0%} ({base_risk:.0%} base + {ai_bonus:.0%} bono IA)\n"
            msg = (
                f"🚀 **Ejecutando Señal: {symbol}**\n\n"
                f"• **Side:** {side}\n"
                f"• **Leverage:** {leverage}x\n"
                f"{risk_info}"
                f"• **Entry (Market):** ~{mark_price}\n"
                f"• **SL:** {signal.get('sl', 'N/A')}\n"
                f"• **TP:** {signal.get('tp', 'N/A')}\n\n"
                f"💡 **Razonamiento:**\n{rationale_msg}"
            )
            try:
                await telegram_context.bot.send_message(
                    chat_id=s['owner_chat_id'],
                    text=msg,
                    parse_mode=None, # Safer: AI rationale often breaks Markdown
                    reply_to_message_id=reply_to_id if chat_id == s['owner_chat_id'] else None
                )
            except Exception as e:
                logger.error(f"Telegram Markdown error: {e}. Sending as plain text.")
                await telegram_context.bot.send_message(
                    chat_id=s['owner_chat_id'],
                    text=msg,
                    parse_mode=None,
                    reply_to_message_id=reply_to_id if chat_id == s['owner_chat_id'] else None
                )
        # Note: In a real scenario we'd need the owner's chatId. 
        # For now, let's assume we reply to where we can if it's the bot handler, 
        # or we log it. Since monitor_channel doesn't have a 'context' bot easy, 
        # we'll use a global bot instance or just log it.
        # Actually, let's try to pass the application instance if possible.

        # 2. Get Symbol Info for Precision
        info = client.futures_exchange_info()
        symbol_info = next((s for s in info['symbols'] if s['symbol'] == symbol), None)
        if not symbol_info:
            logger.error(f"Symbol {symbol} not found on Binance.")
            return

        price_filter = next(f for f in symbol_info['filters'] if f['filterType'] == 'PRICE_FILTER')
        lot_size = next(f for f in symbol_info['filters'] if f['filterType'] == 'LOT_SIZE')
        notional_filter = next((f for f in symbol_info['filters'] if f['filterType'] == 'MIN_NOTIONAL'), None)
        
        tick_size = float(price_filter['tickSize'])
        step_size = float(lot_size['stepSize'])
        # Dynamic minimum notional (default to 5.5 if not found, add 10% buffer)
        min_notional_limit = float(notional_filter['notional']) if notional_filter else 5.0
        min_notional_with_buffer = min_notional_limit * 1.1 

        # 3. Adjust Leverage & Margin Type
        try:
            client.futures_change_margin_type(symbol=symbol, marginType=s['margin_type'])
        except BinanceAPIException as e:
            if "No need to change margin type" not in str(e):
                logger.warning(f"Error changing margin type: {e}")

        client.futures_change_leverage(symbol=symbol, leverage=leverage)

        # 4. Calculate Quantity
        balance_res = client.futures_account_balance()
        usdt_balance = float(next(item for item in balance_res if item["asset"] == "USDT")['balance'])
        
        # Quantity = (Balance * TotalRisk * Leverage) / Price
        raw_quantity = (usdt_balance * total_risk * leverage) / mark_price
        quantity = round_step_size(raw_quantity, step_size)

        if quantity <= 0:
            err_msg = f"❌ **Error en trade {symbol}**: La cantidad calculada es 0. Revisa tu balance en Binance."
            logger.error(err_msg)
            if telegram_context and s.get('owner_chat_id'):
                await telegram_context.bot.send_message(chat_id=s['owner_chat_id'], text=err_msg, parse_mode='Markdown')
            return

        # --- DYNAMIC MINIMUM NOTIONAL CHECK ---
        notional_value = quantity * mark_price
        if notional_value < min_notional_with_buffer:
            logger.info(f"Notional {notional_value} below {min_notional_with_buffer} USDT. Adjusting quantity...")
            quantity = round_step_size(min_notional_with_buffer / mark_price, step_size)
            notional_value = quantity * mark_price
            logger.info(f"Adjusted Qty: {quantity} (Notional: {notional_value})")
            if telegram_context and s.get('owner_chat_id'):
                try:
                    await telegram_context.bot.send_message(
                        chat_id=s['owner_chat_id'],
                        text=f"⚖️ **Ajuste de Notional**: El mínimo de Binance para {symbol} es {min_notional_limit} USDT. Se ajustó la posición a `{quantity}`.",
                        parse_mode=None
                    )
                except:
                    await telegram_context.bot.send_message(
                        chat_id=s['owner_chat_id'],
                        text=f"⚖️ Ajuste de Notional: Se ajustó la posición a {quantity} ({symbol})."
                    )

        logger.info(f"Executing Market Entry. Qty: {quantity}")

        # 5. Send Entry Order
        order_side = 'BUY' if side == 'LONG' else 'SELL'
        entry_order = client.futures_create_order(
            symbol=symbol,
            side=order_side,
            type='MARKET',
            quantity=quantity
        )
        logger.info(f"Entry set: {entry_order['orderId']}")

        # 6. Set SL and TP (if available in signal)
        sl_price_raw = signal.get('sl')
        tp_price_raw = signal.get('tp')
        exit_side = 'SELL' if order_side == 'BUY' else 'BUY'

        if sl_price_raw:
            sl_price = float(sl_price_raw)
            # Validation: SL must not trigger immediately
            is_valid_sl = (order_side == 'BUY' and sl_price < mark_price) or (order_side == 'SELL' and sl_price > mark_price)
            
            if is_valid_sl:
                client.futures_create_order(
                    symbol=symbol,
                    side=exit_side,
                    type='STOP_MARKET',
                    stopPrice=round_step_size(sl_price, tick_size),
                    closePosition=True
                )
                logger.info(f"Stop Loss set at {sl_price}")
            else:
                logger.warning(f"Invalid SL {sl_price} for {order_side} (Price: {mark_price}). Skipping SL.")
                if telegram_context and s.get('owner_chat_id'):
                    await telegram_context.bot.send_message(
                        chat_id=s['owner_chat_id'],
                        text=f"⚠️ **SL omitido**: El nivel `{sl_price}` ya fue alcanzado o es inválido para un `{side}`."
                    )

        if tp_price_raw:
            tp_price = float(tp_price_raw)
            # Validation: TP must not trigger immediately
            is_valid_tp = (order_side == 'BUY' and tp_price > mark_price) or (order_side == 'SELL' and tp_price < mark_price)
            
            if is_valid_tp:
                client.futures_create_order(
                    symbol=symbol,
                    side=exit_side,
                    type='TAKE_PROFIT_MARKET',
                    stopPrice=round_step_size(tp_price, tick_size),
                    closePosition=True
                )
                logger.info(f"Take Profit set at {tp_price}")
            else:
                logger.warning(f"Invalid TP {tp_price} for {order_side} (Price: {mark_price}). Skipping TP.")
                if telegram_context and s.get('owner_chat_id'):
                    await telegram_context.bot.send_message(
                        chat_id=s['owner_chat_id'],
                        text=f"⚠️ **TP omitido**: El nivel `{tp_price}` ya fue alcanzado o es inválido para un `{side}`."
                    )

    except Exception as e:
        logger.error(f"Trade Execution Failed: {str(e)}")
        if telegram_context and s.get('owner_chat_id'):
            await telegram_context.bot.send_message(
                chat_id=s['owner_chat_id'],
                text=f"⚠️ **Error en la ejecución de la operación ({symbol}):**\n`{str(e)}`"
            )

# -----------------------------------------------------------------
# 3. TELEGRAM CHANNEL MONITOR (USER ACCOUNT)
# -----------------------------------------------------------------

async def research_token(symbol, telegram_context=None):
    """Researches BTC (Macro) + Target Token for a given signal."""
    logger.info(f"Researching Macro Context (BTC) and Target ({symbol})...")
    results = []
    
    # 0. Quick DDGS check for BTC (Always included as Macro Context)
    try:
        with DDGS() as ddgs:
            btc_news = ddgs.news("Bitcoin price action crypto", max_results=2)
            results.append("### MACRO CONTEXT (BTC):\n")
            for r in (list(btc_news) or []):
                results.append(f"- BTC News: {r.get('title', '')}")
    except:
        pass

    # 1. Reddit Research via DDGS (robust fallback)
    try:
        with DDGS() as ddgs:
            reddit_query = f"site:reddit.com {symbol} crypto sentiment"
            res = list(ddgs.text(reddit_query, max_results=3)) or []
            if res:
                results.append(f"\n### TARGET RESEARCH ({symbol} from Reddit/Web):\n")
                for r in res:
                    results.append(f"- {r.get('title', '')}")
            else:
                results.append(f"\n### TARGET RESEARCH ({symbol}): No se encontraron discusiones recientes.")
    except Exception as e:
        logger.error(f"Target research failed: {e}")

    return "\n".join(results) if len(results) > 1 else "No se encontró información macro/target suficiente."

async def analyze_signal_with_ai(image_bytes=None, text_context=None, research_data=""):
    """Uses Gemini to classify and analyze market messages."""
    try:
        prompt = (
            "Eres un Senior Quantitative Trader y Experto Analista de Mercados.\n\n"
            "TU OBJETIVO:\n"
            "Analizar un mensaje de Telegram (texto e imagen) y clasificarlo en una de tres categorías:\n"
            "1. SIGNAL: Una señal clara con intención de trade inmediato (Long/Short). Debe tener niveles sugeridos (Entry, SL, TP).\n"
            "2. COMMENTARY: Un análisis, noticia, mensaje educativo o comentario sobre el mercado que no es un trade inmediato.\n"
            "3. IRRELEVANT: Saludos, spam, mensajes de resultados pasados (#symbol + %), o contenido no relacionado.\n\n"
            "REGLAS CRÍTICAS DE CLASIFICACIÓN:\n"
            "- **ZONAS AZULES (VISION)**: En el gráfico, identifica rectángulos azules (Demand Zones).\n"
            "   - **UNEXPLORED (SIGNAL)**: Si la zona azul está vacía de velas (el precio aún no ha llegado o está rebotando justo en el borde) y proyecta un movimiento futuro, es una SEÑAL.\n"
            "   - **EXPLORED (IRRELEVANT/RESULT)**: Si la zona azul ya tiene velas dentro atravesándola o el movimiento ya ocurrió, es un RESULTADO pasado o un análisis de historia. NO operes esto.\n"
            "- **PATRÓN DE RESULTADOS**: Mensajes como '#par +50%' o similares con gráficos que muestran el movimiento ya realizado deben ser marcados como IRRELEVANT.\n"
            "- **MENSAJES EDUCATIVOS**: Si el mensaje describe estructuras (ej. Falling Wedge, Double Bottoms), clasifícalo como COMMENTARY y genera un INSIGHT valioso.\n"
            "- **BTC MACRO**: Si el sentimiento macro de BTC es muy bajista, no valides LONGs en Alts a menos que la configuración de la Blue Zone Unexplored sea perfecta.\n\n"
            "FUENTES DE INFORMACIÓN:\n"
            "1. INVESTIGACIÓN INDEPENDIENTE (Macro BTC + Target): {research_data}\n"
            "2. IMAGEN DEL CHART: Busca la 'Blue Zone' y determina si está 'Explored' o 'Unexplored'.\n"
            "3. MENSAJE ORIGINAL: {text_context}\n\n"
            "FORMATO DE SALIDA (ESTRICTO - SIN MARKDOWN EN ETIQUETAS):\n"
            "TYPE: [SIGNAL / COMMENTARY / IRRELEVANT]\n"
            "JSON: {'symbol': '...', 'side': '...', 'leverage': ..., 'entry': ..., 'sl': ..., 'tp': ..., 'risk_bonus': 0.X} (Solo si es SIGNAL, sino '{}')\n"
            "INSIGHT: (Obligatorio si es SIGNAL o COMMENTARY) Análisis detallado de por qué el post es valioso o qué estructura describe.\n"
            "RATIONALE: (Solo si es SIGNAL) Por qué validas el trade, mencionando específicamente el estado de la 'Blue Zone'."
        )
        
        contents = [prompt]
        if image_bytes:
            img = Image.open(io.BytesIO(image_bytes))
            contents.append(img)
        if text_context:
            contents.append(f"MENSAJE ORIGINAL: {text_context}")

        response = client_genai.models.generate_content(
            model=model_id,
            contents=contents
        )
        return response.text
    except Exception as e:
        logger.error(f"Error Gemini: {e}")
        return None

async def monitor_channel(application):
    """Listens to a specific channel using Telethon."""
    api_id_raw = os.getenv('TELEGRAM_API_ID')
    api_hash = os.getenv('TELEGRAM_API_HASH')
    
    if not api_id_raw or not api_hash:
        logger.error("TELEGRAM_API_ID o TELEGRAM_API_HASH faltan en el .env")
        return

    try:
        api_id = int(api_id_raw)
    except ValueError:
        logger.error("TELEGRAM_API_ID debe ser un número entero.")
        return

async def process_message_logic(text, image_data=None, reply_to_id=None, chat_id=None, application=None):
    """Core logic to analyze message and execute trade if it's a signal."""
    try:
        # User wants ALL messages to be read by Gemini. No hardcoded filters.
        # We only ignore completely empty messages.
        if not text.strip() and not image_data:
            return

        text_lower = text.lower()
        symbol = "BTC" # Default
        if "#" in text_lower:
            parts = text_lower.split("#")
            if len(parts) > 1:
                raw_symbol = parts[1].split()[0].upper()
                symbol = "".join(c for c in raw_symbol if c.isalnum())
        elif image_data and not text_lower:
            symbol = "UNKNOWN"
        
        logger.info(f"Processing message for {symbol} (Chat: {chat_id})")
        
        # 1. Research
        research_data = await research_token(symbol, telegram_context=application)
        
        # 2. AI Analysis
        ai_output = await analyze_signal_with_ai(
            image_bytes=image_data, 
            text_context=text, 
            research_data=research_data
        )
        
        logger.info(f"AI Output: {ai_output}")
        
        if ai_output:
            s = load_settings()
            
            # Normalize for parsing
            ai_clean = ai_output.replace("*", "").upper()
            
            # Parse Tiers
            tier = "IRRELEVANT"
            if "TYPE: SIGNAL" in ai_clean or "TYPE:SIGNAL" in ai_clean: tier = "SIGNAL"
            elif "TYPE: COMMENTARY" in ai_clean or "TYPE:COMMENTARY" in ai_clean: tier = "COMMENTARY"
            
            if tier == "IRRELEVANT":
                # Fallback: if it has an INSIGHT/RATIONALE and no TYPE, try to guess
                if "INSIGHT:" in ai_clean: tier = "COMMENTARY"
                elif "JSON: {" in ai_clean and '"SYMBOL"' in ai_clean: tier = "SIGNAL"
                else:
                    logger.info("Message classified as IRRELEVANT. Ignoring.")
                    return

            if tier == "COMMENTARY":
                insight = "No insight provided."
                if "INSIGHT:" in ai_output:
                    insight = ai_output.split("INSIGHT:")[1].split("RATIONALE:")[0].split("JSON:")[0].strip()
                
                log_daily_action(f"MARKET INSIGHT: {insight[:100]}...")

                if application and s.get('owner_chat_id'):
                    try:
                        await application.bot.send_message(
                            chat_id=s['owner_chat_id'],
                            text=f"🧐 **Market Insight**\n\n{insight}",
                            parse_mode=None,
                            reply_to_message_id=reply_to_id if chat_id == s['owner_chat_id'] else None
                        )
                    except:
                        pass
                return

            # --- SIGNAL Handling ---
            json_str = "{}"
            rationale = "No rationale provided."
            
            if "RATIONALE:" in ai_output:
                rationale = ai_output.split("RATIONALE:")[1].strip()
            if "```json" in ai_output:
                json_str = ai_output.split("```json")[1].split("```")[0].strip()
            elif "```" in ai_output:
                json_str = ai_output.split("```")[1].split("```")[0].strip()
            elif "JSON:" in ai_output:
                json_str = ai_output.split("JSON:")[1].split("INSIGHT:")[0].split("RATIONALE:")[0].strip()

            if not json_str or json_str == "{}":
                logger.info("AI Decided to SKIP this signal.")
                if application and s.get('owner_chat_id'):
                    try:
                        await application.bot.send_message(
                            chat_id=s['owner_chat_id'],
                            text=f"📉 **Señal de {symbol} Ignorada**\n\n{rationale}",
                            parse_mode=None, # Safer
                            reply_to_message_id=reply_to_id if chat_id == s['owner_chat_id'] else None
                        )
                    except:
                        await application.bot.send_message(
                            chat_id=s['owner_chat_id'],
                            text=f"📉 Señal de {symbol} Ignorada\n\n{rationale}",
                            parse_mode=None,
                            reply_to_message_id=reply_to_id if chat_id == s['owner_chat_id'] else None
                        )
                return

            try:
                # Handle single quotes from AI output
                data = json.loads(json_str)
            except json.JSONDecodeError:
                try:
                    import ast
                    data = ast.literal_eval(json_str)
                except Exception as e:
                    logger.error(f"Failed to parse JSON/Dict from AI: {json_str} - Error: {e}")
                    if application and s.get('owner_chat_id'):
                        await application.bot.send_message(
                            chat_id=s['owner_chat_id'],
                            text=f"⚠️ **Error al procesar la respuesta de la IA:**\nEl formato del mensaje no es válido para ejecución automática.\n\n`{json_str}`"
                        )
                    return

            if data and data.get('symbol'):
                await execute_trade(
                    data, 
                    rationale_msg=rationale, 
                    telegram_context=application,
                    reply_to_id=reply_to_id,
                    chat_id=chat_id
                )
                log_daily_action(f"SIGNAL EXECUTED: {data.get('symbol')} {data.get('side')} @ {data.get('entry')}")
    except Exception as e:
        logger.error(f"Error in process_message_logic: {e}")
        s = load_settings()
        if application and s.get('owner_chat_id'):
            try:
                await application.bot.send_message(
                    chat_id=s['owner_chat_id'],
                    text=f"❌ **Error crítico en la lógica de procesamiento:**\n`{str(e)}`"
                )
            except:
                pass

async def monitor_channel(application):
    api_id = os.getenv('TELEGRAM_API_ID')
    api_hash = os.getenv('TELEGRAM_API_HASH')
    
    # Create a Telethon client (User Session) - Using a unique name to avoid locks
    client = TelegramClient('user_session', api_id, api_hash)

    # Start client
    phone = os.getenv('TELEGRAM_PHONE')
    await client.start(phone=phone)
    logger.info("Telethon connected successfully (User Session).")

    # Get self IDs to ignore self-messages
    me = await client.get_me()
    user_me_id = me.id
    bot_id = application.bot.id

    # Security filter enabled for the correct channel ID
    @client.on(events.NewMessage(chats=TARGET_CHANNEL_ID))
    async def handler(event):
        try:
            # 0. IGNORE SELF-MESSAGES (ONLY THE BOT TO PREVENT LOOPS)
            sender_id = event.sender_id
            
            if sender_id == bot_id:
                logger.debug(f"Ignoring message sent by the bot itself (ID {sender_id})")
                return
            
            # Note: We ALLOW event.out and user_me_id so the human owner can trigger the bot
            # in monitored channels.
            
            # Robust Timestamp Filtering
            msg_date = event.date
            if not msg_date.tzinfo:
                msg_date = msg_date.replace(tzinfo=timezone.utc)
            else:
                msg_date = msg_date.astimezone(timezone.utc)

            if msg_date < STARTUP_TIME:
                logger.debug(f"Ignoring historical message ({msg_date} < {STARTUP_TIME})")
                return

            # Deduplication
            msg_id = f"telethon_{event.id}"
            if msg_id in PROCESSED_MESSAGES:
                logger.info(f"Skipping already processed Telethon message: {msg_id}")
                return
            PROCESSED_MESSAGES.add(msg_id)

            chat_id = event.chat_id
            msg_text = (event.text or "").strip()
            
            # ABSOLUTELY LOG EVERYTHING
            logger.info(f"📩 RAW EVENT RECEIVED: Peer={event.peer_id} ChatID={chat_id} Text='{msg_text[:50]}'")

            # 0. Deduplication (Legacy/Secondary)
            if event.id in PROCESSED_MESSAGES:
                return
            PROCESSED_MESSAGES.add(event.id)
            save_processed_messages(PROCESSED_MESSAGES)
            
            # Limit set size
            if len(PROCESSED_MESSAGES) > 1000:
                ordered = list(PROCESSED_MESSAGES)
                PROCESSED_MESSAGES.clear()
                PROCESSED_MESSAGES.update(ordered[-500:])
                save_processed_messages(PROCESSED_MESSAGES)

            # 1. Download photo if exists
            image_data = None
            if event.photo:
                path = await event.download_media()
                with open(path, 'rb') as f:
                    image_data = f.read()
                os.remove(path)

            await process_message_logic(
                text=msg_text,
                image_data=image_data,
                reply_to_id=event.id,
                chat_id=chat_id,
                application=application
            )

        except Exception as e:
            logger.error(f"Error in Telethon handler: {e}")

    logger.info("Starting Heartbeat...")
    
    async def heartbeat():
        while True:
            logger.info("💓 Telethon Heartbeat: Client is still connected and listening...")
            try:
                # Try to manually fetch the last message to check visibility
                messages = await client.get_messages(TARGET_CHANNEL_ID, limit=1)
                if messages:
                    msg = messages[0]
                    logger.info(f"🔍 DIAGNOSTIC: Last message in Target Channel ({TARGET_CHANNEL_ID}): '{msg.text[:50]}'")
                else:
                    logger.info(f"🔍 DIAGNOSTIC: Target Channel ({TARGET_CHANNEL_ID}) returned NO messages.")
            except Exception as e:
                logger.error(f"🔍 DIAGNOSTIC ERROR: Cannot read target channel: {e}")
            await asyncio.sleep(30)

    # Use the current event loop
    loop = asyncio.get_event_loop()
    loop.create_task(heartbeat())
    
    await client.run_until_disconnected()

# -----------------------------------------------------------------
# 3. MAIN EXECUTION
# -----------------------------------------------------------------

async def bot_message_handler(update: Update, context: ContextTypes.DEFAULT_TYPE):
    """Handles messages sent directly to the bot via its token."""
    # Robust Timestamp Filtering
    msg_date = update.message.date
    if not msg_date.tzinfo:
        msg_date = msg_date.replace(tzinfo=timezone.utc)
    else:
        msg_date = msg_date.astimezone(timezone.utc)

    if msg_date < STARTUP_TIME:
        logger.info(f"Ignoring historical Bot message from {msg_date}")
        return

    # IGNORE SELF-MESSAGES
    if update.message.from_user.id == context.application.bot.id:
        return

    # Deduplication
    msg_id = f"botapi_{update.message.message_id}"
    if msg_id in PROCESSED_MESSAGES:
        logger.info(f"Skipping already processed Bot message: {msg_id}")
        return
    PROCESSED_MESSAGES.add(msg_id)

    s = load_settings()
    # Security: only process if it's the owner or if we want to allow others (let's stick to owner for now)
    if not update.effective_chat or update.effective_chat.id != s.get('owner_chat_id'):
        return

    # Skip if it's a command (already handled)
    if update.message.text and update.message.text.startswith('/'):
        return

    msg_text = update.message.text or update.message.caption or ""
    
    # Check for photo
    image_data = None
    if update.message.photo:
        photo_file = await update.message.photo[-1].get_file()
        image_bytes = await photo_file.download_as_bytearray()
        image_data = bytes(image_bytes)

    await process_message_logic(
        text=msg_text,
        image_data=image_data,
        reply_to_id=update.message.message_id,
        chat_id=update.effective_chat.id,
        application=context.application
    )

async def main():
    # 1. Verificación de conexión a Binance al inicio
    check_binance_connection()

    # Start the standard Bot for status commands
    token = os.getenv('TELEGRAM_TOKEN')
    application = ApplicationBuilder().token(token).post_init(post_init).build()
    application.add_handler(CommandHandler('start', start))
    application.add_handler(CommandHandler('balance', balance))
    application.add_handler(CommandHandler('settings', get_settings_handler))
    application.add_handler(CommandHandler('set_risk', set_risk))
    application.add_handler(CommandHandler('set_leverage', set_leverage))
    application.add_handler(CommandHandler('set_margin', set_margin))
    
    # Add general message handler for the bot
    from telegram.ext import MessageHandler, filters
    application.add_handler(MessageHandler(filters.TEXT | filters.PHOTO, bot_message_handler))

    # Initialize bot components
    async with application:
        await application.initialize()
        await application.start()
        if application.updater:
            await application.updater.start_polling()
            logger.info("Standard Bot (Polling) started.")
        
        # Run Telethon monitor
        try:
            await monitor_channel(application)
        except asyncio.CancelledError:
            pass
        finally:
            if application.updater:
                await application.updater.stop()
            await application.stop()

if __name__ == '__main__':
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        pass
