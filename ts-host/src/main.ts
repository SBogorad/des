import init, { WasmEngine } from 'rzd-wasm';

async function start() {
    await init();
    
    // Default network data (could be fetched)
    const networkData = {
        stations: {},
        segments: {},
        legs: {},
        roads: {}
    };

    const engine = new WasmEngine(JSON.stringify(networkData), 42);
    
    document.getElementById('app')!.innerHTML = `
        <div class="dashboard">
            <h1>RZD Synthetic DES Engine</h1>
            <div id="status">Engine Initialized</div>
            <button id="runBtn">Run 24h Simulation</button>
            <pre id="output"></pre>
        </div>
    `;

    document.getElementById('runBtn')!.addEventListener('click', () => {
        const result = engine.run_simulation(1440);
        document.getElementById('output')!.textContent = result;
        document.getElementById('status')!.textContent = 'Simulation Finished';
    });
}

start();
