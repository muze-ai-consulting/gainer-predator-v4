#!/bin/bash
set -e # Abortar si falla cualquier comando
# Script de despliegue para TarzanesTrader (HFT Sniper)

# 1. Configuración
IMAGE_NAME="robvandam12/trz-bot:latest"
SERVER_IP="13.208.216.213"
KEY_PATH="tarzanestrader.pem"

echo "🔐 Verificando login en Docker Hub..."
docker login || exit 1

echo "🚀 Iniciando build para ARM64 (Arquitectura nativa Mac-AWS)..."
# Dado que ambos son ARM64, un build normal es más compatible
docker build -t $IMAGE_NAME .
docker push $IMAGE_NAME

echo "✅ Imagen subida a Docker Hub: $IMAGE_NAME"

echo "📡 Conectando a AWS Osaka para desplegar..."
ssh -i $KEY_PATH ubuntu@$SERVER_IP << EOF
  # Pull de la imagen (debe estar logueado en AWS)
  docker pull $IMAGE_NAME
  
  # Detener y limpiar contenedor anterior
  docker stop binance_sniper || true
  docker rm binance_sniper || true
  
  # Correr con volumen para persistencia y .env
  docker run -d \
    --name binance_sniper \
    --restart unless-stopped \
    -v /home/ubuntu/.env:/app/.env \
    -v /home/ubuntu/bot_state.json:/app/bot_state.json \
    -v /home/ubuntu/binance_sniper.session:/app/binance_sniper.session \
    $IMAGE_NAME
  
  echo "✅ Bot desplegado y corriendo en AWS Osaka."
EOF
