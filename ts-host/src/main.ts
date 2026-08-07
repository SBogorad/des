import { RzdSimulator } from './simulator';

const sim = new RzdSimulator();
let networkJson = '';
let scenarioJson = '';

const $ = (id: string) => document.getElementById(id)!;

async function initEngine() {
  const seed = parseInt(($('seed') as HTMLInputElement).value, 10) || 42;
  await sim.init(seed);
  $('status').textContent = 'Ready';
  $('initBtn').setAttribute('disabled', 'true');
  ($('loadNetworkBtn') as HTMLButtonElement).disabled = false;
  ($('loadScenarioBtn') as HTMLButtonElement).disabled = false;
  ($('runBtn') as HTMLButtonElement).disabled = true;
}

function loadNetwork() {
  const val = ($('networkInput') as HTMLTextAreaElement).value.trim();
  if (!val) { alert('Paste network JSON first'); return; }
  try { JSON.parse(val); } catch { alert('Invalid JSON'); return; }
  networkJson = val;
  sim.loadNetwork(networkJson);
  $('networkStatus').textContent = 'Network loaded: ' + JSON.parse(val).stations ? Object.keys(JSON.parse(val).stations).length + ' stations' : 'OK';
  ($('loadScenarioBtn') as HTMLButtonElement).disabled = false;
}

function loadScenario() {
  const val = ($('scenarioInput') as HTMLTextAreaElement).value.trim();
  if (!val) { alert('Paste scenario JSON first'); return; }
  try { JSON.parse(val); } catch { alert('Invalid JSON'); return; }
  scenarioJson = val;
  sim.loadScenario(scenarioJson);
  $('scenarioStatus').textContent = 'Scenario loaded';
  const offset = parseInt(($('shiftOffset') as HTMLInputElement).value, 10) || 60;
  const interval = parseInt(($('shiftInterval') as HTMLInputElement).value, 10) || 30;
  sim.scheduleShifts(offset, interval);
  const warmup = parseInt(($('warmup') as HTMLInputElement).value, 10) || 0;
  if (warmup > 0) sim.setWarmup(warmup);
  $('scheduleStatus').textContent = `Shifts scheduled (offset=${offset}, interval=${interval})`;
  ($('runBtn') as HTMLButtonElement).disabled = false;
}

async function runSimulation() {
  const minutes = parseInt(($('runMinutes') as HTMLInputElement).value, 10) || 1440;
  const t0 = performance.now();
  const result = sim.run(minutes);
  const elapsed = ((performance.now() - t0) / 1000).toFixed(2);
  const summary = sim.getSummary();

  $('output').textContent = JSON.stringify(JSON.parse(result), null, 2);
  $('status').textContent = `Simulation finished in ${elapsed}s`;

  const crewInfo = Object.keys(JSON.parse(sim.getCrewState('') ? '{}' : '{}'));
  $('results').innerHTML = `
    <h3>Results</h3>
    <table>
      <tr><td>Duration</td><td>${elapsed}s</td></tr>
      <tr><td>Crews</td><td>${summary.crew_count}</td></tr>
      <tr><td>Locos</td><td>${summary.loco_count}</td></tr>
      <tr><td>TU3 Rows</td><td>${summary.tu3_rows}</td></tr>
      <tr><td>Violations</td><td>${summary.total_violations}</td></tr>
      <tr><td>Current Time</td><td>${summary.current_time} min</td></tr>
    </table>
  `;

  ($('exportBtn') as HTMLButtonElement).disabled = false;
  ($('exportTsvBtn') as HTMLButtonElement).disabled = false;
}

function exportCed() {
  const ced = sim.exportCed();
  const blob = new Blob([JSON.stringify(ced, null, 2)], { type: 'application/json' });
  downloadBlob(blob, `ced_export_${ced.simulation_id}.json`);
}

function exportTsv() {
  const tsv = sim.exportTu3Tsv();
  const blob = new Blob([tsv], { type: 'text/tab-separated-values' });
  downloadBlob(blob, 'tu3_export.tsv');
}

function downloadBlob(blob: Blob, filename: string) {
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = filename;
  a.click();
  URL.revokeObjectURL(url);
}

function render() {
  $('app').innerHTML = `
    <div class="dashboard">
      <h1>RZD DES Engine — Северная СДТ</h1>
      <div class="card">
        <h2>1. Initialize</h2>
        <label>Seed: <input type="number" id="seed" value="42" min="0"/></label>
        <button id="initBtn">Init Engine</button>
        <span id="status">Not initialized</span>
      </div>
      <div class="card">
        <h2>2. Load Network</h2>
        <textarea id="networkInput" rows="4" placeholder='Paste network JSON here...'></textarea>
        <button id="loadNetworkBtn" disabled>Load Network</button>
        <span id="networkStatus"></span>
      </div>
      <div class="card">
        <h2>3. Load Scenario</h2>
        <textarea id="scenarioInput" rows="4" placeholder='Paste scenario JSON here...'></textarea>
        <button id="loadScenarioBtn" disabled>Load & Schedule</button>
        <span id="scenarioStatus"></span>
        <div class="row">
          <label>Shift offset: <input type="number" id="shiftOffset" value="60" size="5"/></label>
          <label>Shift interval: <input type="number" id="shiftInterval" value="30" size="5"/></label>
          <label>Warmup (min): <input type="number" id="warmup" value="0" size="5"/></label>
        </div>
        <span id="scheduleStatus"></span>
      </div>
      <div class="card">
        <h2>4. Run</h2>
        <label>Simulate (min): <input type="number" id="runMinutes" value="1440" size="6"/></label>
        <button id="runBtn" disabled>Run</button>
        <span id="runStatus"></span>
      </div>
      <div id="results"></div>
      <pre id="output"></pre>
      <div class="card">
        <button id="exportBtn" disabled>Export CED JSON</button>
        <button id="exportTsvBtn" disabled>Export TU3 TSV</button>
      </div>
    </div>
  `;

  $('initBtn').addEventListener('click', initEngine);
  $('loadNetworkBtn').addEventListener('click', loadNetwork);
  $('loadScenarioBtn').addEventListener('click', loadScenario);
  $('runBtn').addEventListener('click', runSimulation);
  $('exportBtn').addEventListener('click', exportCed);
  $('exportTsvBtn').addEventListener('click', exportTsv);
}

document.addEventListener('DOMContentLoaded', render);
