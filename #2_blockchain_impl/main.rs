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
mod block;
use block::*;
use crate::blockchain::*;

mod blockchain;


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

    let behaviour = peer::AppBehaviour::new(Blockchain::new(), response_sender, init_sender.clone()).await;

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
