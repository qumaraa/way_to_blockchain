use chrono::prelude::*;
use libp2p::{
    core::upgrade,
    futures::StreamExt,
    mplex,
    noise::{Keypair, NoiseConfig, X25519Spec},
    swarm::{Swarm, SwarmBuilder},
    tcp::TokioTcpConfig,
    Transport,
};
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::Duration;
use tokio::{
    io::{stdin, AsyncBufReadExt, BufReader},
    select, spawn,
    sync::mpsc,
    time::sleep,
};

const DIFFICULTY_PREFIX: &str = "00";

mod peer;
mod key;
mod transaction;
use transaction::Transaction;
mod mempool;

pub struct App {
    pub mining_reward: f32,
    pub blocks: Vec<Block>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Block {
    pub id: u64,
    pub hash: String,
    pub previous_hash: String,
    pub timestamp: i64,
    pub data: String,
    pub transactions: Vec<Transaction>,
    pub nonce: u64,
}

impl Block {
    pub fn new(id: u64, previous_hash: String, data: String, transactions: Vec<Transaction>) -> Self {
        let now = Utc::now();
        let (nonce, hash) = mine_block(id, now.timestamp(), &previous_hash, &data);
        Self {
            id,
            hash,
            timestamp: now.timestamp(),
            previous_hash,
            data,
            nonce,
            transactions,
        }
    }
}

fn calculate_hash(id: u64, timestamp: i64, previous_hash: &str, data: &str, nonce: u64) -> Vec<u8> {
    let data = serde_json::json!({
        "id": id,
        "previous_hash": previous_hash,
        "data": data,
        "timestamp": timestamp,
        "nonce": nonce
    });
    let mut hasher = Sha256::new();
    hasher.update(data.to_string().as_bytes());
    hasher.finalize().as_slice().to_owned()
}

fn mine_block(id: u64, timestamp: i64, previous_hash: &str, data: &str) -> (u64, String) {
    info!("mining block...");
    let mut nonce = 0;

    loop {
        if nonce % 100000 == 0 {
            info!("nonce: {}", nonce);
        }
        let hash = calculate_hash(id, timestamp, previous_hash, data, nonce);
        let binary_hash = hash_to_binary_representation(&hash);
        if binary_hash.starts_with(DIFFICULTY_PREFIX) {
            info!(
                "mined! nonce: {}, hash: {}, binary hash: {}",
                nonce,
                hex::encode(&hash),
                binary_hash
            );
            return (nonce, hex::encode(hash));
        }
        nonce += 1;
    }
}

fn hash_to_binary_representation(hash: &[u8]) -> String {
    let mut res: String = String::default();
    for c in hash {
        res.push_str(&format!("{:b}", c));
    }
    res
}

impl App {
    fn new() -> Self {
        Self { mining_reward: 10.0, blocks: vec![] }
    }

    fn genesis(&mut self) {
        let genesis_block = Block {
            id: 0,
            timestamp: Utc::now().timestamp(),
            previous_hash: String::from("0000000000000000000000000000000000000000000000000000000000000000"),
            nonce: 0,
            hash: "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
            data: "Genesis".to_string(),
            transactions: vec![],
        };
        self.blocks.push(genesis_block);
    }

    fn try_add_block(&mut self,block: Block) {
        let latest_block = self.blocks.last().expect("there is at least one block.");
        if self.is_block_valid(&block, latest_block) {
            self.blocks.push(block);
        }else {
            error!("could not add block - invalid");
        }
    }

    fn is_block_valid(&self, block: &Block, previous_block: &Block) -> bool {
        if block.previous_hash != previous_block.hash {
            warn!("block with id#{} has wrong previous hash",block.id);
            return false;
        }else if !hash_to_binary_representation(
            &hex::decode(&block.hash).expect("can decode from hex"),
        ).starts_with(DIFFICULTY_PREFIX){
            warn!(
                "block with id#{} is not the next block after the latest: {}",
                block.id, previous_block.id
            );
            return false;
        }else if hex::encode(calculate_hash(
            block.id,
            block.timestamp,
            &block.previous_hash,
            &block.data,
            block.nonce,
        )) != block.hash
        {
            warn!("block with id#{} has invalid hash",block.id);
            return false;
        }
        true
    }
    fn is_chain_valid(&self, chain: &[Block]) -> bool {
        for i in 0..chain.len() {
            if i == 0 {
                continue; // skip the genesis block
            }
            let first = chain.get(i - 1).expect("has to exist");
            let second = chain.get(i).expect("has to exist");
            if !self.is_block_valid(second, first) {
                return false;
            }
        }
        true
    }
    fn choose_chain(&mut self,local: Vec<Block>, remote: Vec<Block>) -> Vec<Block> {
        let is_local_valid = self.is_chain_valid(&local);
        let is_remote_valid = self.is_chain_valid(&remote);

        if is_local_valid && is_remote_valid {
            if local.len() >= remote.len() {
                local
            }else {
                remote
            }
        }else if is_remote_valid && !is_local_valid {
            remote
        }else if !is_remote_valid && is_local_valid {
            local
        }else {
            panic!("local and remote chains are both invalid");
        }
    }
}
#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    ///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
    /*
     * Здесь настраивается транспорт для обмена данными между узлами. Используется TCP для обеспечения соединения между узлами.
     * Шифрование и аутентификация осуществляются с использованием протокола шума (Noise Protocol Framework).
     * Mplex используется для мультиплексирования потоков данных. Создается поведение приложения (AppBehaviour), которое определяет, как узлы взаимодействуют друг с другом.
     */


    info!("Peer Id: {}", peer::PEER_ID.clone());
    let (response_sender, mut response_rcv) = mpsc::unbounded_channel();
    let (init_sender, mut init_rcv) = mpsc::unbounded_channel();

    let auth_keys = Keypair::<X25519Spec>::new()
        .into_authentic(&peer::KEYS)
        .expect("can create auth keys");

    let transp = TokioTcpConfig::new()
        .upgrade(upgrade::Version::V1)
        .authenticate(NoiseConfig::xx(auth_keys).into_authenticated())
        .multiplex(mplex::MplexConfig::new())
        .boxed();

    let behaviour = peer::AppBehaviour::new(App::new(), response_sender, init_sender.clone()).await;

    let mut swarm = SwarmBuilder::new(transp, behaviour, *peer::PEER_ID)
        .executor(Box::new(|fut| {
            spawn(fut);
        }))
        .build();
    //////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
    let mut stdin = BufReader::new(stdin()).lines();

    Swarm::listen_on(
        &mut swarm,
        "/ip4/0.0.0.0/tcp/0"
            .parse()
            .expect("can get a local socket"),
    )
        .expect("swarm can be started");
    ////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
    /*
    Здесь создается и настраивается экземпляр Swarm, который представляет собой множество узлов,
    способных обмениваться сообщениями между собой. Swarm является основной структурой для P2P взаимодействия.
     */
    spawn(async move {
        sleep(Duration::from_secs(1)).await;
        info!("sending init event");
        init_sender.send(true).expect("can send init event");
    });
    ///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
    loop {
        /*
        Этот блок кода создает переменную evt, которая представляет собой результат обработки различных событий.
        Он использует макрос select! из библиотеки tokio, который позволяет асинхронно обрабатывать
        несколько потенциальных источников событий. В данном случае обрабатываются следующие типы событий:

         * Ввод пользователя с клавиатуры (stdin.next_line()).
         * Получение ответа от другого узла (response_rcv.recv()).
         * Получение инициализационного события (init_rcv.recv()).
         * События от Swarm (swarm.select_next_some()).
         */
        let evt = {
            select! {
                line = stdin.next_line() => Some(peer::EventType::Input(line.expect("can get line").expect("can read line from stdin"))),
                response = response_rcv.recv() => {
                    Some(peer::EventType::LocalChainResponse(response.expect("response exists")))
                },
                _init = init_rcv.recv() => {
                    Some(peer::EventType::Init)
                }
                event = swarm.select_next_some() => {
                    info!("Unhandled Swarm Event: {:?}", event);
                    None
                },
            }
        };
        ////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
        if let Some(event) = evt {
            /*
            Если произошло какое-либо событие, то выполняется соответствующий блок кода внутри match.
            Например, если событие является инициализационным (peer::EventType::Init),
            выполняется блок кода, предназначенный для этого типа события.

            Обработка конкретных типов событий:
            Внутри каждого варианта события (peer::EventType::Init, peer::EventType::LocalChainResponse, peer::EventType::Input) выполняются соответствующие действия в зависимости от типа события. Например:

             * Если тип события - инициализация (peer::EventType::Init), то выполняется блок кода для инициализации узла, отправки запроса цепи блоков другому узлу и т.д.
             * Если тип события - ответ от локальной цепи блоков (peer::EventType::LocalChainResponse), то этот ответ публикуется в сеть через протокол floodsub.
             * Если тип события - ввод пользователя (peer::EventType::Input), то выполняются различные команды, такие как вывод списка узлов сети, вывод цепи блоков или создание нового блока.
             */
            match event {
                peer::EventType::Init => {
                    let peers = peer::get_list_peers(&swarm);
                    swarm.behaviour_mut().app.genesis();

                    info!("connected nodes: {}", peers.len());
                    if !peers.is_empty() {
                        let req = peer::LocalChainRequest {
                            from_peer_id: peers
                                .iter()
                                .last()
                                .expect("at least one peer")
                                .to_string(),
                        };

                        let json = serde_json::to_string(&req).expect("can jsonify request");
                        swarm
                            .behaviour_mut()
                            .floodsub
                            .publish(peer::CHAIN_TOPIC.clone(), json.as_bytes());
                    }
                }
                peer::EventType::LocalChainResponse(resp) => {
                    let json = serde_json::to_string(&resp).expect("can jsonify response");
                    swarm
                        .behaviour_mut()
                        .floodsub
                        .publish(peer::CHAIN_TOPIC.clone(), json.as_bytes());
                }
                peer::EventType::Input(line) => match line.as_str() {
                    "ls p" => peer::handle_print_peers(&swarm),
                    cmd if cmd.starts_with("ls c") => peer::handle_print_chain(&swarm),
                    cmd if cmd.starts_with("create b") => peer::handle_create_block(cmd, &mut swarm),
                    cmd if cmd.starts_with("send") => peer::handle_add_transaction(cmd,&swarm),
                    _ => error!("unknown command"),
                },
            }
        }
    }
}
