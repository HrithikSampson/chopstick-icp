use candid::{CandidType, Decode, Deserialize, Encode, Principal};
use ic_cdk::api::caller;
use ic_cdk_macros::{init, query, update};
use ic_stable_structures::memory_manager::{MemoryId, MemoryManager, VirtualMemory};
use ic_stable_structures::{DefaultMemoryImpl, StableBTreeMap, Storable};
use rand::Rng;
use std::collections::HashMap;
use uuid::Uuid;
use std::{borrow::Cow, cell::RefCell}; 

#[derive(CandidType, Deserialize, Debug, Clone)]
struct Player {
    id: Principal,
    game: Option<String>,
    left_hand: u8,
    right_hand: u8,
}

#[derive(CandidType, Deserialize, Debug, PartialEq, Clone)]
enum GameState {
    WaitingForPlayer,
    InProgress,
    Finished { winner: Principal },
}

#[derive(CandidType, Deserialize, Debug, Clone)]
enum Turn {
    Player1,
    Player2,
}

#[derive(CandidType, Deserialize, Debug, Clone)]
struct Game {
    session_id: String,
    player1: Player,
    player2: Option<Player>,
    state: GameState,
    current_turn: Turn,
    // Additional fields to represent the state of the game
}

#[derive(Default, CandidType, Deserialize)]
struct ChopsticksGameService {
    games: HashMap<String, Game>,

}

impl Storable for ChopsticksGameService {
    fn to_bytes(&self) -> Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }
    const BOUND: ic_stable_structures::storable::Bound = ic_stable_structures::storable::Bound::Bounded { max_size: 10000, is_fixed_size: true };
}


impl Storable for Game {
    fn to_bytes(&self) -> Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }

    const BOUND: ic_stable_structures::storable::Bound = ic_stable_structures::storable::Bound::Bounded { max_size: 10000, is_fixed_size: false };
}



thread_local! {
    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> =
        RefCell::new(MemoryManager::init(DefaultMemoryImpl::default()));
    static GAME_SERVICE: RefCell<StableBTreeMap<String, ChopsticksGameService, VirtualMemory<DefaultMemoryImpl>>> = RefCell::new(
        StableBTreeMap::init(
            MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(0))),
        )
    );
}

#[init]
fn init() {
    let init_state = ChopsticksGameService {
        games: HashMap::new(),
    };
    GAME_SERVICE.with(|service| {
        service.borrow_mut().insert("chopsticks_game_service".to_string(), init_state);
    });
}
impl Game {
    fn new() -> Self {
        let mut rng = rand::thread_rng();
        Game {
            session_id: Uuid::new_v4().to_string(),
            player1: Player { id: caller(), game: None, left_hand: 1, right_hand: 1 },
            player2: None,
            state: GameState::WaitingForPlayer,
            current_turn: if rng.gen() { Turn::Player1 } else { Turn::Player2 },
            // Initialize hands, other fields as necessary
        }
    }

    fn join(&mut self, player: Player) {
        if self.state == GameState::WaitingForPlayer && self.player2.is_none() {
            self.player2 = Some(player);
            self.state = GameState::InProgress;
        }
    }

    fn make_move(&mut self, player_id: Principal, hand: u8, target_hand: u8) {
        if self.state != GameState::InProgress {
            return;
        }

        // Determine if it's player1's or player2's turn and if the move is valid
        let (active_player, opponent) = match self.current_turn {
            Turn::Player1 if self.player1.id == player_id => (&mut self.player1, self.player2.as_mut()),
            Turn::Player2 if self.player2.as_ref().map_or(false, |p| p.id == player_id) => (self.player2.as_mut().unwrap(), Some(&mut self.player1)),
            _ => return, // Not the player's turn or player not found
        };

        // Assuming hand and target_hand are 0 for left hand and 1 for right hand, adjust as needed
        let active_hand = if hand == 0 { active_player.left_hand } else { active_player.right_hand };
        if active_hand == 0 { return; } // Cannot make a move with an inactive hand

        if let Some(opponent) = opponent {
            let opponent_hand = if target_hand == 0 { &mut opponent.left_hand } else { &mut opponent.right_hand };
            *opponent_hand += active_hand;
            if *opponent_hand >= 5 { *opponent_hand = 0; } // Reset hand if it reaches the threshold

            // Check if the game has ended
            if opponent.left_hand == 0 && opponent.right_hand == 0 {
                self.state = GameState::Finished { winner: player_id };
            } else {
                // Switch turns
                self.current_turn = match self.current_turn {
                    Turn::Player1 => Turn::Player2,
                    Turn::Player2 => Turn::Player1,
                };
            }
        }
    }
}

#[update]
fn start_game() -> Result<String, String> {
    let game = Game::new();
    let session_id = game.session_id.clone();
    GAME_SERVICE.with(|service| {
        let mut games = service.borrow_mut().get(&"chopsticks_game_service".to_string());
        if let Some(mut game_service) = games {
            
            game_service.games.insert(session_id.clone(), game);
        }
        else{

        }
    });
    Ok(session_id)
}

#[update]
fn join_game(session_id: String) -> Result<(), String> {
    let player = Player { id: caller(), game: Some(session_id.clone()) , left_hand:1 ,right_hand: 1};
    GAME_SERVICE.with(|service| {
        let mut games = service.borrow_mut().get(&"chopsticks_game_service".to_string());
        if let Some(mut game_service) = games {
            if let Some(game) = game_service.games.get_mut(&session_id) {
                game.join(player);
                Ok(())
            } else {
                Err("Game not found".to_string())
            }
        }
        else {
            Ok(())
        }
    })
}

#[update]
fn make_move(session_id: String, hand: u8, target_hand: u8) -> Result<(), String> {
    let player_id = caller();
    GAME_SERVICE.with(|service| {
        
        let mut games = service.borrow_mut().get(&"chopsticks_game_service".to_string());
        if let Some(mut game_service) = games {
            if let Some(mut game) = game_service.games.get_mut(&session_id) {
                game.make_move(player_id, hand, target_hand);
                Ok(())
            } else {
                Err("Game not found".to_string())
            }
        }
        else {
            Ok(())
        }
    })
}

#[query]
fn get_game_state(session_id: String) -> Result<Game, String> {
    GAME_SERVICE.with(|service| {
        let games = service.borrow();
        if let Some(game_service) = games.get(&"chopsticks_game_service".to_string()) {
            if let Some(game) = game_service.games.get(session_id.as_str()){
                Ok(game.clone())
            }
            else{
                Err("Game not found".to_string())
            }
        } else {
            Err("Game not found".to_string())
        }
    })
}

// Export the candid interface
ic_cdk_macros::export_candid!();
