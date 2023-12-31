import init, { run, start_game, set_input_active } from "./ascii_bomb_ecs_lib.js";

let wasm_loaded = false;
async function start_wasm() {
    await init();
    wasm_loaded = true;

    try {
        run();
    } catch (e) {
        // the winit crate throws an exception for control flow, which should be ignored
        if (!e.message.includes("This isn't actually an error!")) {
            console.error(e);
            document.getElementById('button-box').remove();
            document.getElementById('game-container').remove();
            document.getElementById('error-screen').removeAttribute("hidden");
        }
    }
}
start_wasm();

const isTouchDevice = 'ontouchstart' in document.documentElement;

let canvasContainerWidth = undefined;
let canvasContainerHeight = undefined;
let canvas = document.getElementById('bevy-canvas');
function updateCanvasSize() {
    if (canvasContainerHeight != undefined && canvasContainerWidth != undefined) {
        if (canvasContainerHeight / canvasContainerWidth > canvas.offsetHeight / canvas.offsetWidth) {
            canvas.setAttribute('style', 'width:' + canvasContainerWidth + 'px');
        } else {
            canvas.setAttribute('style', 'height:' + canvasContainerHeight + 'px');
        }
    }
}

// override winit resize requests
new ResizeObserver(() => {
    updateCanvasSize();
}).observe(canvas);

let isPortrait = undefined;
let gameContainer = document.getElementById('game-container');
let canvasContainer = document.getElementById('canvas-container');
let canvasInnerContainer = document.getElementById('canvas-inner-container');
function updateCanvasContainerSize() {
    let controls = document.getElementById('controls');
    if (controls != undefined) {
        const rem = parseInt(getComputedStyle(document.documentElement).fontSize);

        const gameContainerWidth = gameContainer.offsetWidth;
        const gameContainerHeight = gameContainer.offsetHeight;

        const controlsPortraitHeightLandscapeWidth = 35 * rem;

        const canvasContainerLandscapeWidth = gameContainer.offsetWidth - 37 * rem;
        const canvasContainerPortraitHeight = gameContainer.offsetHeight - 37 * rem;

        const portraitCanvasContainerIsMoreSquare = Math.abs(canvasContainerPortraitHeight - gameContainerWidth) <= Math.abs(gameContainerHeight - canvasContainerLandscapeWidth);

        // should the orientation change?
        if ((isPortrait || isPortrait == undefined) && !portraitCanvasContainerIsMoreSquare) {
            isPortrait = false;

            canvasInnerContainer.style.display = 'flex';
            canvasInnerContainer.style.justifyContent = 'left';

            controls.style.float = 'right';
            controls.style.padding = '0 1rem';
        } else if ((!isPortrait || isPortrait == undefined) && portraitCanvasContainerIsMoreSquare) {
            isPortrait = true;

            canvasInnerContainer.style.display = 'block';
            canvasInnerContainer.style.justifyContent = 'center';

            controls.style.float = 'none';
            controls.style.padding = '1rem 0';
        }

        if (isPortrait) {
            canvasContainer.style.width = gameContainerWidth + 'px';
            canvasContainer.style.height = canvasContainerPortraitHeight + 'px';

            controls.style.width = gameContainerWidth + 'px';
            controls.style.height = controlsPortraitHeightLandscapeWidth + 'px';
        } else {
            canvasContainer.style.width = canvasContainerLandscapeWidth + 'px';
            canvasContainer.style.height = gameContainerHeight + 'px';

            controls.style.width = controlsPortraitHeightLandscapeWidth + 'px';
            controls.style.height = gameContainerHeight + 'px';
        }
    }

    canvasContainerHeight = canvasContainer.offsetHeight;
    canvasContainerWidth = canvasContainer.offsetWidth;

    updateCanvasSize();
}

window.onresize = updateCanvasContainerSize;

// prevents pinch-to-zoom on iOS Safari
document.addEventListener('touchmove', function (event) {
    event.preventDefault();
}, { passive: false });
// prevents double tap to zoom on iOS Safari
document.addEventListener('dblclick', function (event) {
    event.preventDefault();
}, { passive: false });

window.onload = () => {
    // prevent non-number room ID input
    var roomIdInput = document.getElementById('roomID');
    roomIdInput.addEventListener('keypress', function (event) {
        const isNumber = isFinite(event.key);
        if (!isNumber) {
            event.preventDefault();
            return false;
        }
    });
}

function toggleCustomMatchboxServerSettings() {
    var checkbox = document.getElementById("customMatchboxServerCheckbox");
    var settings = document.getElementById("matchboxServerSettings");

    if (checkbox.checked == true) {
        settings.style.display = "block";
    } else {
        settings.style.display = "none";
    }
}
window.toggleCustomMatchboxServerSettings = toggleCustomMatchboxServerSettings

function toggleCustomICEServerSettings() {
    var checkbox = document.getElementById("customICEServerCheckbox");
    var settings = document.getElementById("ICEServerSettings");

    if (checkbox.checked == true) {
        settings.style.display = "block";
    } else {
        settings.style.display = "none";
    }
}
window.toggleCustomICEServerSettings = toggleCustomICEServerSettings

function startGame() {
    var number_of_players = parseInt(document.getElementById("numberInput").value);
    var room_id = document.getElementById("roomID").value;
    var use_custom_matchbox_server_settings = document.getElementById('customMatchboxServerCheckbox').checked;
    var matchbox_server_url = "";
    var use_custom_ice_server_settings = document.getElementById('customICEServerCheckbox').checked;
    var ice_server_url = "";
    var turn_server_username = "";
    var turn_server_credential = "";

    // Validate player count input
    if (number_of_players < 2 || number_of_players > 8) {
        alert("Please enter a player count between 2 and 8.");
        return;
    }

    if (room_id.trim() !== "") {
        if (!/^([0-9]{4})$/.test(room_id)) {
            alert("Invalid room ID.");
            return;
        }
    } else {
        room_id = "quick_join";
    }

    // Validate custom Matchbox server settings
    if (use_custom_matchbox_server_settings == true) {
        matchbox_server_url = document.getElementById("matchboxServerURL").value;

        if (matchbox_server_url.trim() === "") {
            alert("Please enter a Matchbox server URL.");
            return;
        }
    }

    // Validate custom ICE server settings
    if (use_custom_ice_server_settings == true) {
        ice_server_url = document.getElementById("iceServerURL").value;
        turn_server_username = document.getElementById("iceServerUsername").value;
        turn_server_credential = document.getElementById("iceServerCredential").value;

        if (ice_server_url.trim() === "") {
            alert("Please enter a STUN/TURN server URL.");
            return;
        }
    }

    console.log("Number of players: " + number_of_players);
    console.log("Room ID: " + room_id);
    if (use_custom_matchbox_server_settings) {
        console.log("Matchbox server URL: " + matchbox_server_url);
    }
    if (use_custom_ice_server_settings) {
        console.log("STUN/TURN server URL: " + ice_server_url);
        console.log("TURN server username: " + turn_server_username);
        console.log("TURN server credential: " + turn_server_credential);
    }

    document.getElementById('button-box').remove();
    document.getElementById('game-container').removeAttribute("hidden");

    if (isTouchDevice) {
        if (document.fullscreenEnabled) {
            // go fullscreen
            let elem = document.documentElement;
            if (elem.requestFullscreen) {
                elem.requestFullscreen();
            } else if (elem.webkitRequestFullscreen) { /* Safari */
                elem.webkitRequestFullscreen();
            } else if (elem.msRequestFullscreen) { /* IE11 */
                elem.msRequestFullscreen();
            }
        }
        else {
            // disable the fullscreen button
            let elem = document.getElementById('button-fullscreen');
            elem.classList.add('grey');
            elem.removeAttribute('onpointerclick');
        }
    } else {
        // remove on-screen controls
        document.getElementById('controls').remove();

        canvasContainer.setAttribute('style', 'height:100%');
        canvasContainer.style.heigth = '100%';
    }

    updateCanvasContainerSize();

    canvas.focus();
    start_game(number_of_players, room_id, matchbox_server_url, ice_server_url, turn_server_username, turn_server_credential);
}
window.startGame = startGame

function setInputActive(input) {
    set_input_active(input);
}
window.setInputActive = setInputActive

function toggleFullscreen() {
    if (!document.fullscreenElement &&    // alternative standard method
        !document.mozFullScreenElement && !document.webkitFullscreenElement) {  // current working methods
        if (document.documentElement.requestFullscreen) {
            document.documentElement.requestFullscreen();
        } else if (document.documentElement.mozRequestFullScreen) {
            document.documentElement.mozRequestFullScreen();
        } else if (document.documentElement.webkitRequestFullscreen) {
            document.documentElement.webkitRequestFullscreen(Element.ALLOW_KEYBOARD_INPUT);
        }
    } else {
        if (document.cancelFullScreen) {
            document.cancelFullScreen();
        } else if (document.mozCancelFullScreen) {
            document.mozCancelFullScreen();
        } else if (document.webkitCancelFullScreen) {
            document.webkitCancelFullScreen();
        }
    }
}
window.toggleFullscreen = toggleFullscreen
