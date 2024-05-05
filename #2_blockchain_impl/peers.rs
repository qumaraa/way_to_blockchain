//! # P2P Сеть Блокчейна
//!
//! Этот модуль реализует P2P сеть для приложения блокчейна, используя фреймворк libp2p.
//!
//! ## Обзор
//!
//! P2P сеть состоит из узлов, которые обмениваются сообщениями между собой с использованием протокола floodsub для распространения сообщений и mDNS для обнаружения узлов.
//!
//! ## Модули
//!
//! - `main.rs`: Точка входа в приложение, настраивает P2P сеть.
//!
//! ## Структуры и Типы
//!
//! - `ChainResponse`: Структура, представляющая ответ на запрос цепочки блоков.
//! - `LocalChainRequest`: Структура, представляющая запрос на получение локальной цепочки блоков.
//! - `EventType`: Перечисление, определяющее типы событий, которые могут возникнуть в приложении.
//! - `AppBehaviour`: Поведение сетевого узла приложения, включающее floodsub и mDNS.
//!
//! ## Функции
//!
//! - `get_list_peers`: Получает список узлов в сети.
//! - `handle_print_peers`: Выводит список узлов в лог.
//! - `handle_print_chain`: Выводит локальную цепочку блоков в лог.
//! - `handle_create_block`: Создает новый блок и транслирует его в сеть.
//!
//! ## Методы
//!
//! ### `AppBehaviour`
//!
//! - `new`: Создает новый экземпляр `AppBehaviour`.
//!
//! ### `NetworkBehaviourEventProcess` для `AppBehaviour`
//!
//! - `inject_event`: Обрабатывает входящие события floodsub и mDNS.
//!
//! ### `NetworkBehaviourEventProcess` для `MdnsEvent`
//!
//! - `inject_event`: Обрабатывает события mDNS, такие как обнаружение и истечение срока узлов.

use super::{App, Block};
use libp2p::{
    floodsub::{Floodsub, FloodsubEvent, Topic},
    identity,
    mdns::{Mdns, MdnsEvent},
    swarm::{NetworkBehaviourEventProcess, Swarm},
    NetworkBehaviour, PeerId,
};
use log::{error, info};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use tokio::sync::mpsc;
use crate::transaction::Transaction;

pub static KEYS: Lazy<identity::Keypair> = Lazy::new(identity::Keypair::generate_ed25519);
pub static PEER_ID: Lazy<PeerId> = Lazy::new(|| PeerId::from(KEYS.public()));
pub static CHAIN_TOPIC: Lazy<Topic> = Lazy::new(|| Topic::new("chains"));
pub static BLOCK_TOPIC: Lazy<Topic> = Lazy::new(|| Topic::new("blocks"));

#[derive(Debug, Serialize, Deserialize)]
pub struct ChainResponse {
    pub blocks: Vec<Block>,
    pub receiver: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LocalChainRequest {
    pub from_peer_id: String,
}

pub enum EventType {
    LocalChainResponse(ChainResponse),
    Input(String),
    Init,
}


#[derive(NetworkBehaviour)]
pub struct AppBehaviour {
    //     floodsub: Это компонент, который реализует протокол floodsub для обмена сообщениями в P2P сети.
    //          * Floodsub используется для широковещательной передачи сообщений по темам (topics) в сети.
    //     Он позволяет вашему узлу отправлять и принимать сообщения о новых блоках, запросах цепочки блоков и других событиях в сети.
    //          * mdns: Это компонент, который обеспечивает механизм обнаружения узлов в локальной сети с использованием Multicast DNS (mDNS).
    //     Он позволяет вашему узлу обнаруживать другие узлы в локальной сети без необходимости использования централизованных серверов обнаружения.
    //     response_sender: Это отправитель сообщений, который используется для отправки ответов на запросы, связанные с цепочкой блоков.
    //     Например, когда ваш узел получает запрос на получение локальной цепочки блоков от другого узла,
    //     он может использовать этот отправитель, чтобы отправить ответ с текущей локальной цепочкой блоков.
    //          * init_sender: Это отправитель сообщений, который используется для отправки инициализационных событий.
    //     Например, при запуске вашего узла он может отправить инициализационное событие для сигнализации другим узлам, что он готов к работе.
    //           * app: Это структура, которая представляет блокчейна. Она содержит логику приложения,
    //     такую как хранение блоков, обработка новых блоков и выбор цепочки блоков. В AppBehaviour она используется для доступа к функциональности приложения из сетевого поведения.
    pub floodsub: Floodsub,
    pub mdns: Mdns,
    #[behaviour(ignore)]
    pub response_sender: mpsc::UnboundedSender<ChainResponse>,
    #[behaviour(ignore)]
    pub init_sender: mpsc::UnboundedSender<bool>,
    #[behaviour(ignore)]
    pub app: App,
}

impl AppBehaviour {
    pub async fn new(
        app: App,
        response_sender: mpsc::UnboundedSender<ChainResponse>,
        init_sender: mpsc::UnboundedSender<bool>,
    ) -> Self {
        let mut behaviour = Self {
            app,
            floodsub: Floodsub::new(*PEER_ID),
            mdns: Mdns::new(Default::default())
                .await
                .expect("can create mdns"),
            response_sender,
            init_sender,
        };
        behaviour.floodsub.subscribe(CHAIN_TOPIC.clone());
        behaviour.floodsub.subscribe(BLOCK_TOPIC.clone());

        behaviour
    }
}

// incoming event handler
impl NetworkBehaviourEventProcess<FloodsubEvent> for AppBehaviour {
    fn inject_event(&mut self, event: FloodsubEvent) {
        if let FloodsubEvent::Message(msg) = event {
            if let Ok(resp) = serde_json::from_slice::<ChainResponse>(&msg.data) {
                if resp.receiver == PEER_ID.to_string() {
                    info!("Response from {}:", msg.source);
                    resp.blocks.iter().for_each(|r| info!("{:?}", r));

                    self.app.blocks = self.app.choose_chain(self.app.blocks.clone(), resp.blocks);
                }
            } else if let Ok(resp) = serde_json::from_slice::<LocalChainRequest>(&msg.data) {
                info!("sending local chain to {}", msg.source.to_string());
                let peer_id = resp.from_peer_id;
                if PEER_ID.to_string() == peer_id {
                    if let Err(e) = self.response_sender.send(ChainResponse {
                        blocks: self.app.blocks.clone(),
                        receiver: msg.source.to_string(),
                    }) {
                        error!("error sending response via channel, {}", e);
                    }
                }
            } else if let Ok(block) = serde_json::from_slice::<Block>(&msg.data) {
                info!("received new block from {}", msg.source.to_string());
                self.app.try_add_block(block);
            }
        }
    }
}

impl NetworkBehaviourEventProcess<MdnsEvent> for AppBehaviour {
    fn inject_event(&mut self, event: MdnsEvent) {
        match event {
            MdnsEvent::Discovered(discovered_list) => {
                for (peer, _addr) in discovered_list {
                    self.floodsub.add_node_to_partial_view(peer);
                }
            }
            MdnsEvent::Expired(expired_list) => {
                for (peer, _addr) in expired_list {
                    if !self.mdns.has_node(&peer) {
                        self.floodsub.remove_node_from_partial_view(&peer);
                    }
                }
            }
        }
    }
}

pub fn get_list_peers(swarm: &Swarm<AppBehaviour>) -> Vec<String> {
    info!("Discovered Peers:");
    let nodes = swarm.behaviour().mdns.discovered_nodes();
    let mut unique_peers = HashSet::new();
    for peer in nodes {
        unique_peers.insert(peer);
    }
    unique_peers.iter().map(|p| p.to_string()).collect()
}

pub fn handle_print_peers(swarm: &Swarm<AppBehaviour>) {
    let peers = get_list_peers(swarm);
    peers.iter().for_each(|p| info!("{}", p));
}

pub fn handle_add_transaction(cmd: &str,swarm: &Swarm<AppBehaviour>) {
    info!("handle_add_transaction()");
}

pub fn handle_print_chain(swarm: &Swarm<AppBehaviour>) {
    info!("Local Blockchain:");
    let pretty_json =
        serde_json::to_string_pretty(&swarm.behaviour().app.blocks).expect("can jsonify blocks");
    info!("{}", pretty_json);
}

pub fn handle_create_block(cmd: &str, swarm: &mut Swarm<AppBehaviour>) {
    if let Some(data) = cmd.strip_prefix("create b") {
        let behaviour = swarm.behaviour_mut();
        let transaction1 = Transaction {
            amount: 10.0,
            sender: "03638e59237924128f9c9be55d435ecfcac3c6f774641b1cf24873ebbacede6098".to_string(),
            receiver: "a8668a61f0d237403fb31545eaa0dcd756dc33a609ecfcc777c8cb2c6dce8247".to_string(),
            //signature: "".to_string(),
        };

        let transaction2 = Transaction {
            amount: 10.0,
            sender: "03638e5op1239dnvcnrkdf39rk435ecfcac3c6f774641b1cf24873ebbacede6098".to_string(),
            receiver: "a8668a61ffwef213lasddgtvnb9329rjd4s67aapsfkcfln777c8cb2c6dce8247".to_string(),
            //signature: "".to_string(),
        };
        let collect_tx: Vec<Transaction> = vec![transaction1,transaction2];
        let latest_block = behaviour
            .app
            .blocks
            .last()
            .expect("there is at least one block");
        let block = Block::new(
            latest_block.id + 1,
            latest_block.hash.clone(),
            data.to_owned(),
            collect_tx,
        );
        let json = serde_json::to_string(&block).expect("can jsonify request");
        behaviour.app.blocks.push(block);
        info!("broadcasting new block");
        behaviour
            .floodsub
            .publish(BLOCK_TOPIC.clone(), json.as_bytes());
    }
}

/*
{
    "id": 0,
    "hash": "0000000000000000000000000000000000000000000000000000000000000000",
    "previous_hash": "0000000000000000000000000000000000000000000000000000000000000000",
    "timestamp": 1714932210,
    "data": "Genesis",
    "transactions": [],
    "nonce": 0
  },
  {
    "id": 1,
    "hash": "00007a4d65472a95794e905ad8426baa62be51b789ec3864a24fbf9e5dcaceb3",
    "previous_hash": "0000000000000000000000000000000000000000000000000000000000000000",
    "timestamp": 1714932213,
    "data": "",
    "transactions": [
      {
        "sender": "03638e59237924128f9c9be55d435ecfcac3c6f774641b1cf24873ebbacede6098",
        "receiver": "a8668a61f0d237403fb31545eaa0dcd756dc33a609ecfcc777c8cb2c6dce8247",
        "amount": 10.0
      },
      {
        "sender": "03638e5op1239dnvcnrkdf39rk435ecfcac3c6f774641b1cf24873ebbacede6098",
        "receiver": "a8668a61ffwef213lasddgtvnb9329rjd4s67aapsfkcfln777c8cb2c6dce8247",
        "amount": 10.0
      }
    ],
    "nonce": 232558
  }
]

*/
