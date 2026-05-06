import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import "./App.css";

const STEPS = {
  INIT: 0,
  DOTNET_ERROR: 1,
  MAIN_MENU: 2,
  DOWNLOAD_METHOD: 3,
  MORRENUS_FLOW: 4,
  MANUAL_FLOW: 5,
  DEPOT_SELECT: 6,
  CRACK_FLOW: 7,
  STEAMLESS_CONFIG: 8,
  STEAMLESS_RUNNING: 9,
  GOLDBERG_CONFIG: 10,
  GOLDBERG_RUNNING: 11,
};

function App() {
  const [step, setStep] = useState(STEPS.INIT);
  const [prevStep, setPrevStep] = useState(null);
  const [sourcePath, setSourcePath] = useState("");
  const [apiKey, setApiKey] = useState(localStorage.getItem("morrenus_api_key") || "");
  const [downloadDir, setDownloadDir] = useState(localStorage.getItem("download_dir") || "downloads");
  const [appId, setAppId] = useState("");
  
  // Game Info State
  const [gameInfo, setGameInfo] = useState(null);
  const [searchQuery, setSearchQuery] = useState("");
  const [searchResults, setSearchResults] = useState([]);
  const [showSearch, setShowSearch] = useState(false);
  const [dbExists, setDbExists] = useState(false);
  const [updatingDb, setUpdatingDb] = useState(false);
  
  // Depot Selection State
  const [availableDepots, setAvailableDepots] = useState([]);
  const [selectedDepots, setSelectedDepots] = useState([]);
  const [preparing, setPreparing] = useState(false);
  
  // Steamless State
  const [gameFolder, setGameFolder] = useState("");
  const [scannedExes, setScannedExes] = useState([]);
  const [selectedExes, setSelectedExes] = useState([]);
  const [steamlessOptions, setSteamlessOptions] = useState({
    quiet: false,
    keep_bind: false,
    keep_stub: false,
    dump_payload: false,
    dump_drmp: false,
    realign: true,
    recalc_checksum: true,
    experimental: false,
  });
  const [steamlessAvailable, setSteamlessAvailable] = useState(false);
  const [steamlessResults, setSteamlessResults] = useState(null);
  
  // Goldberg State
  const [goldbergOptions, setGoldbergOptions] = useState({
    account_name: "Goldberg",
    steam_id: "76561197960287930",
    language: "english",
    listen_port: 47584,
    custom_broadcast_ip: "",
    disable_networking: false,
    offline_mode: false,
    enable_overlay: false,
    use_experimental: false,
    custom_save_location: "",
    generate_interfaces: false,
    force_generate_all: false,
  });
  const [goldbergAvailable, setGoldbergAvailable] = useState(false);
  const [goldbergLanguages, setGoldbergLanguages] = useState([]);
  const [useCustomBroadcast, setUseCustomBroadcast] = useState(false);
  const [useCustomSave, setUseCustomSave] = useState(false);
  const [goldbergResults, setGoldbergResults] = useState(null);

  const [fromDownload, setFromDownload] = useState(false); // Track if we came from download flow
  
  // App State
  const [loading, setLoading] = useState(false);
  
  // Console & Progress State
  const [logs, setLogs] = useState([]);
  const [progress, setProgress] = useState(0);
  const [currentFile, setCurrentFile] = useState("");
  const logsEndRef = useRef(null);
  const searchTimeoutRef = useRef(null);

  useEffect(() => {
    checkDotnet();
    checkDb();
    checkSteamless();
    checkGoldberg();
    fetchGoldbergLanguages();
  }, []);

  useEffect(() => {
    if (logsEndRef.current) {
      logsEndRef.current.scrollIntoView({ behavior: "smooth" });
    }
  }, [logs]);

  // Listen for download logs
  useEffect(() => {
    const unlisten = listen("download-log", (event) => {
      const msg = event.payload;
      setLogs((prev) => [...prev, msg]);
      
      const percentMatch = msg.match(/^\s*(\d{1,3}\.\d{2})%/);
      if (percentMatch) {
        setProgress(parseFloat(percentMatch[1]));
        const parts = msg.split("%");
        if (parts.length > 1) {
          setCurrentFile(parts[1].trim());
        }
      } else if (msg.includes("Total downloaded:")) {
        setProgress(100);
        setCurrentFile("Completed.");
      }
    });

    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  // Listen for steamless logs
  useEffect(() => {
    const unlisten = listen("steamless-log", (event) => {
      const msg = event.payload;
      setLogs((prev) => [...prev, msg]);
    });

    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  // Listen for goldberg logs
  useEffect(() => {
    const unlisten = listen("goldberg-log", (event) => {
      const msg = event.payload;
      setLogs((prev) => [...prev, msg]);
    });

    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  // Fetch game info when appId changes
  useEffect(() => {
    if (appId && appId.match(/^\d+$/)) {
      fetchGameInfo(appId);
    } else {
      setGameInfo(null);
    }
  }, [appId]);

  // Debounced search
  useEffect(() => {
    if (searchQuery.length < 2) {
      setSearchResults([]);
      return;
    }
    
    if (searchTimeoutRef.current) {
      clearTimeout(searchTimeoutRef.current);
    }
    
    searchTimeoutRef.current = setTimeout(async () => {
      try {
        const results = await invoke("search_games", { query: searchQuery });
        setSearchResults(results);
      } catch (e) {
        console.error("Search failed:", e);
      }
    }, 300);
  }, [searchQuery]);

  async function checkDb() {
    try {
      const exists = await invoke("check_appid_db");
      setDbExists(exists);
    } catch (e) {
      console.error(e);
    }
  }

  async function checkSteamless() {
    try {
      await invoke("check_steamless_available");
      setSteamlessAvailable(true);
    } catch (e) {
      console.error("Steamless not found:", e);
      setSteamlessAvailable(false);
    }
  }

  async function checkGoldberg() {
    try {
      await invoke("check_goldberg_available");
      setGoldbergAvailable(true);
    } catch (e) {
      console.error("Goldberg not found:", e);
      setGoldbergAvailable(false);
    }
  }
  
  async function fetchGoldbergLanguages() {
    try {
      const langs = await invoke("get_goldberg_languages");
      setGoldbergLanguages(langs);
    } catch (e) {
      console.error("Failed to fetch languages", e);
    }
  }

  async function updateDb() {
    setUpdatingDb(true);
    try {
      await invoke("update_appid_db");
      setDbExists(true);
    } catch (e) {
      alert("Failed to update database: " + e);
    }
    setUpdatingDb(false);
  }

  async function fetchGameInfo(id) {
    try {
      const info = await invoke("get_game_info", { appId: id });
      setGameInfo(info);
    } catch (e) {
      console.error(e);
    }
  }

  async function checkDotnet() {
    try {
      const installed = await invoke("check_dotnet");
      if (installed) {
        setStep(STEPS.MAIN_MENU);
      } else {
        setStep(STEPS.DOTNET_ERROR);
      }
    } catch (e) {
      console.error(e);
      setStep(STEPS.DOTNET_ERROR);
    }
  }

  async function selectFolder() {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        defaultPath: downloadDir === "downloads" ? undefined : downloadDir,
      });
      if (selected) {
        setDownloadDir(selected);
      }
    } catch (err) {
      console.error(err);
    }
  }
  
  async function selectSource(type) {
    try {
      const options = { multiple: false };
      if (type === 'folder') {
        options.directory = true;
      } else {
        options.directory = false;
        options.filters = [{ name: 'Zip/Lua', extensions: ['zip', 'lua'] }];
      }
      
      const selected = await open(options);
      if (selected) {
        setSourcePath(selected);
      }
    } catch (err) {
      console.error(err);
    }
  }

  async function selectGameFolder() {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
      });
      if (selected) {
        setGameFolder(selected);
        // Auto-scan for executables
        scanForExecutables(selected);
      }
    } catch (err) {
      console.error(err);
    }
  }

  async function scanForExecutables(folderPath) {
    try {
      const exes = await invoke("scan_game_folder", { folderPath });
      setScannedExes(exes);
      setSelectedExes(exes); // Select all by default
    } catch (e) {
      console.error("Scan failed:", e);
      setScannedExes([]);
      setSelectedExes([]);
    }
  }

  function selectGame(game) {
    setAppId(game.appid);
    setSearchQuery("");
    setSearchResults([]);
    setShowSearch(false);
  }

  async function prepareDownload() {
    if (!appId) {
      alert("Please enter an AppID");
      return;
    }
    
    setPreparing(true);
    localStorage.setItem("download_dir", downloadDir);
    
    try {
      let depots;
      if (step === STEPS.MORRENUS_FLOW) {
        if (!apiKey) throw "Missing API Key";
        localStorage.setItem("morrenus_api_key", apiKey);
        depots = await invoke("prepare_morrenus_download", { appId, apiKey, targetDir: downloadDir });
      } else if (step === STEPS.MANUAL_FLOW) {
        if (!sourcePath) throw "Missing Source Path";
        depots = await invoke("prepare_manual_download", { appId, sourcePath, targetDir: downloadDir });
      }
      
      setAvailableDepots(depots);
      setSelectedDepots(depots.map(d => d.depot_id));
      setPrevStep(step);
      setStep(STEPS.DEPOT_SELECT);
    } catch (e) {
      alert("Preparation failed: " + e);
    }
    setPreparing(false);
  }

  async function executeDownload() {
    if (selectedDepots.length === 0) {
      alert("Please select at least one depot");
      return;
    }
    
    setLoading(true);
    setLogs([]);
    setProgress(0);
    setCurrentFile("Starting...");
    
    try {
      await invoke("execute_selected_download", { selectedDepotIds: selectedDepots });
      setLogs(p => [...p, "Download finished successfully."]);
    } catch (e) {
      setLogs(p => [...p, `Error: ${e}`]);
    }
  }

  async function cancelDownload() {
    try {
      await invoke("cancel_staging");
    } catch (e) {
      console.error(e);
    }
    setStep(prevStep || STEPS.DOWNLOAD_METHOD);
    setAvailableDepots([]);
    setSelectedDepots([]);
  }
  
  function finishProcess() {
    setLoading(false);
    setStep(STEPS.MAIN_MENU);
    setAvailableDepots([]);
    setSelectedDepots([]);
    setFromDownload(false);
    setSteamlessResults(null);
    setGoldbergResults(null);
  }

  async function openGameFolderAction() {
    if (gameFolder) {
      try {
         await invoke("open_explorer_folder", { path: gameFolder });
      } catch(e) {
         console.error("Failed to open folder:", e);
      }
    }
  }

  function continueToCrack() {
    // User finished download y wants to crack
    setFromDownload(true);
    // Set the game folder to the download location
    const gameName = gameInfo?.name?.replace(/[^a-zA-Z0-9 ]/g, '').trim() || `AppID_${appId}`;
    const expectedFolder = `${downloadDir}/${gameName}`;
    setGameFolder(expectedFolder);
    setLoading(false);
    setStep(STEPS.STEAMLESS_CONFIG);
    // Scan the folder
    scanForExecutables(expectedFolder);
  }

  function continueToGoldberg() {
    setLoading(false);
    setStep(STEPS.GOLDBERG_CONFIG);
  }

  function toggleDepot(depotId) {
    setSelectedDepots(prev => {
      if (prev.includes(depotId)) {
        return prev.filter(id => id !== depotId);
      } else {
        return [...prev, depotId];
      }
    });
  }

  function selectAllDepots() {
    setSelectedDepots(availableDepots.map(d => d.depot_id));
  }

  function deselectAllDepots() {
    setSelectedDepots([]);
  }

  function toggleExe(exePath) {
    setSelectedExes(prev => {
      if (prev.includes(exePath)) {
        return prev.filter(p => p !== exePath);
      } else {
        return [...prev, exePath];
      }
    });
  }

  function selectAllExes() {
    setSelectedExes([...scannedExes]);
  }

  function deselectAllExes() {
    setSelectedExes([]);
  }

  function updateSteamlessOption(key, value) {
    setSteamlessOptions(prev => ({ ...prev, [key]: value }));
  }

  async function runSteamless() {
    if (selectedExes.length === 0) {
      alert("Please select at least one executable");
      return;
    }

    setLoading(true);
    setLogs([]);
    setStep(STEPS.STEAMLESS_RUNNING);

    try {
      const result = await invoke("run_steamless_cli", {
        exePaths: selectedExes,
        options: steamlessOptions,
      });
      setSteamlessResults(result);
      setLogs(p => [...p, `\nCompleted: ${result.successful}/${result.total} successful`]);
    } catch (e) {
      setLogs(p => [...p, `Error: ${e}`]);
    }
  }

  // Game Header Component
  const GameHeader = () => {
    if (!gameInfo) return null;
    return (
      <div className="game-header fade-in">
        <img 
          src={gameInfo.header_url} 
          alt={gameInfo.name}
          onError={(e) => { e.target.style.display = 'none'; }}
        />
        <div className="game-header-info">
          <span className="game-name">{gameInfo.name}</span>
          <span className="game-appid">AppID: {gameInfo.appid}</span>
        </div>
      </div>
    );
  };

  // Game Search Component
  const GameSearch = () => (
    <div className="search-container">
      <div className="search-header">
        <span style={{fontSize: '1.2rem'}}>🔍</span>
        <input
          placeholder="Search games by name..."
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
          autoFocus
        />
        <button className="secondary" style={{width: 'auto'}} onClick={() => setShowSearch(false)}>Close</button>
      </div>
      
      {!dbExists && (
        <div className="search-notice">
          <p>Game database not found.</p>
          <button onClick={updateDb} disabled={updatingDb}>
            {updatingDb ? "Downloading..." : "Download Database (~15MB)"}
          </button>
        </div>
      )}
      
      {searchResults.length > 0 && (
        <div className="search-results">
          {searchResults.map((game) => (
            <div key={game.appid} className="search-result-item" onClick={() => selectGame(game)}>
              <img src={game.header_url} alt="" onError={(e) => e.target.style.display='none'} />
              <div className="search-result-info">
                <span className="search-result-name">{game.name}</span>
                <span className="search-result-appid">{game.appid}</span>
              </div>
            </div>
          ))}
        </div>
      )}
      
      {searchQuery.length >= 2 && searchResults.length === 0 && dbExists && (
        <p style={{textAlign: 'center', opacity: 0.5}}>No results found</p>
      )}
    </div>
  );

  function formatBytes(bytes) {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
  }

  function getFileName(fullPath) {
    const parts = fullPath.split(/[/\\]/);
    return parts[parts.length - 1];
  }

  // Depot Selection Component
  const DepotSelection = () => (
    <div className="card fade-in" style={{maxWidth: '600px'}}>
      <h2>Select Depots</h2>
      <GameHeader />
      
      <p style={{fontSize: '0.9em', marginBottom: '1rem'}}>
        Found {availableDepots.length} depot(s). {selectedDepots.length > 1 ? 
          "Files will be organized in separate folders per depot." : 
          "Files will be downloaded to a single folder."}
      </p>
      
      <div className="depot-actions">
        <button className="secondary" style={{fontSize: '0.85rem', padding: '0.5rem 1rem'}} onClick={selectAllDepots}>Select All</button>
        <button className="secondary" style={{fontSize: '0.85rem', padding: '0.5rem 1rem'}} onClick={deselectAllDepots}>Deselect All</button>
      </div>
      
      <div className="depot-list">
        {availableDepots.map((depot) => (
          <label key={depot.depot_id} className="depot-item">
            <input
              type="checkbox"
              checked={selectedDepots.includes(depot.depot_id)}
              onChange={() => toggleDepot(depot.depot_id)}
            />
            <div className="depot-info" style={{width: '100%'}}>
              <div style={{display: 'flex', justifyContent: 'space-between', alignItems: 'center'}}>
                  <span className="depot-id">Depot {depot.depot_id}</span>
                  <span className="depot-os" style={{fontSize: '0.75em', padding: '2px 6px', borderRadius: '4px', background: 'rgba(255,255,255,0.1)'}}>
                    {depot.os}
                  </span>
              </div>
              <div style={{display: 'flex', justifyContent: 'space-between', fontSize: '0.85em', color: '#aaa', marginTop: '4px'}}>
                  <span>Files: {depot.file_count}</span>
                  <span>Size: {formatBytes(depot.total_size)}</span>
              </div>
              <span className="depot-manifest" style={{fontSize: '0.75em', opacity: 0.6, marginTop: '2px'}}>ID: {depot.manifest_id}</span>
            </div>
          </label>
        ))}
      </div>
      
      <div className="spacer"></div>
      <button onClick={executeDownload} disabled={selectedDepots.length === 0}>
        Download {selectedDepots.length} Depot(s)
      </button>
      <div className="spacer"></div>
      <button className="secondary" onClick={cancelDownload}>Cancel</button>
    </div>
  );

  // Steamless Configuration Component
  const SteamlessConfig = () => (
    <div className="card fade-in" style={{maxWidth: '650px'}}>
      <h2> Steamless Configuration</h2>
      
      {!steamlessAvailable && (
        <div className="warning-box">
          <p>️ Steamless CLI not found! Make sure <code>Steamless.CLI.exe</code> is in the application directory.</p>
        </div>
      )}
      
      <div className="section">
        <h3>Game Folder</h3>
        <div className="input-group">
          <input 
            placeholder="Select game installation folder"
            value={gameFolder}
            readOnly
          />
          <button className="secondary" style={{width: 'auto'}} onClick={selectGameFolder}>📂</button>
        </div>
      </div>

      {scannedExes.length > 0 && (
        <div className="section">
          <h3>Executables Found ({scannedExes.length})</h3>
          <div className="depot-actions">
            <button className="secondary" style={{fontSize: '0.85rem', padding: '0.5rem 1rem'}} onClick={selectAllExes}>Select All</button>
            <button className="secondary" style={{fontSize: '0.85rem', padding: '0.5rem 1rem'}} onClick={deselectAllExes}>Deselect All</button>
          </div>
          <div className="exe-list">
            {scannedExes.map((exe) => (
              <label key={exe} className="exe-item">
                <input
                  type="checkbox"
                  checked={selectedExes.includes(exe)}
                  onChange={() => toggleExe(exe)}
                />
                <span className="exe-name" title={exe}>{getFileName(exe)}</span>
              </label>
            ))}
          </div>
        </div>
      )}

      <div className="section">
        <h3>Steamless Options</h3>
        <div className="options-grid">
          <label className="option-item">
            <input
              type="checkbox"
              checked={steamlessOptions.realign}
              onChange={(e) => updateSteamlessOption('realign', e.target.checked)}
            />
            <div className="option-info">
              <span className="option-name">Realign Sections</span>
              <span className="option-desc">Fix section alignment (recommended)</span>
            </div>
          </label>
          
          <label className="option-item">
            <input
              type="checkbox"
              checked={steamlessOptions.recalc_checksum}
              onChange={(e) => updateSteamlessOption('recalc_checksum', e.target.checked)}
            />
            <div className="option-info">
              <span className="option-name">Recalculate Checksum</span>
              <span className="option-desc">Recalc EXE checksum after unpacking</span>
            </div>
          </label>
          
          <label className="option-item">
            <input
              type="checkbox"
              checked={steamlessOptions.keep_bind}
              onChange={(e) => updateSteamlessOption('keep_bind', e.target.checked)}
            />
            <div className="option-info">
              <span className="option-name">Keep .bind Section</span>
              <span className="option-desc">Preserve the .bind section</span>
            </div>
          </label>
          
          <label className="option-item">
            <input
              type="checkbox"
              checked={steamlessOptions.keep_stub}
              onChange={(e) => updateSteamlessOption('keep_stub', e.target.checked)}
            />
            <div className="option-info">
              <span className="option-name">Keep DOS Stub</span>
              <span className="option-desc">Keep original DOS stub</span>
            </div>
          </label>
          
          <label className="option-item">
            <input
              type="checkbox"
              checked={steamlessOptions.dump_payload}
              onChange={(e) => updateSteamlessOption('dump_payload', e.target.checked)}
            />
            <div className="option-info">
              <span className="option-name">Dump Payload</span>
              <span className="option-desc">Extract Steam stub payload</span>
            </div>
          </label>
          
          <label className="option-item">
            <input
              type="checkbox"
              checked={steamlessOptions.dump_drmp}
              onChange={(e) => updateSteamlessOption('dump_drmp', e.target.checked)}
            />
            <div className="option-info">
              <span className="option-name">Dump SteamDRMP</span>
              <span className="option-desc">Extract SteamDRMP.dll</span>
            </div>
          </label>
          
          <label className="option-item">
            <input
              type="checkbox"
              checked={steamlessOptions.quiet}
              onChange={(e) => updateSteamlessOption('quiet', e.target.checked)}
            />
            <div className="option-info">
              <span className="option-name">Quiet Mode</span>
              <span className="option-desc">Suppress debug output</span>
            </div>
          </label>
          
          <label className="option-item">
            <input
              type="checkbox"
              checked={steamlessOptions.experimental}
              onChange={(e) => updateSteamlessOption('experimental', e.target.checked)}
            />
            <div className="option-info">
              <span className="option-name">Experimental</span>
              <span className="option-desc">Enable experimental features</span>
            </div>
          </label>
        </div>
      </div>

      <div className="spacer"></div>
      <button onClick={runSteamless} disabled={!steamlessAvailable || selectedExes.length === 0}>
         Run Steamless on {selectedExes.length} File(s)
      </button>
      <div className="spacer"></div>
      <button className="secondary" onClick={() => setStep(fromDownload ? STEPS.MAIN_MENU : STEPS.CRACK_FLOW)}>
        Back
      </button>
    </div>
  );

  async function runGoldberg(mode = 'apply') {
    if (!gameFolder) {
      alert("Please select a game folder first");
      return;
    }
    
    // Prepare config
    const config = {
      ...goldbergOptions,
      custom_broadcast_ip: useCustomBroadcast ? goldbergOptions.custom_broadcast_ip : null,
      custom_save_location: useCustomSave ? goldbergOptions.custom_save_location : null,
    };
    
    // Use AppID from previous steps if available, otherwise default
    const targetAppId = appId || goldbergOptions.steam_id;
    
    setLoading(true);
    setLogs([]);
    setStep(STEPS.GOLDBERG_RUNNING);
    
    try {
      let result;
      if (mode === 'apply') {
        result = await invoke("apply_goldberg_emulator", {
          gameFolder,
          appId: targetAppId,
          config
        });
        setGoldbergResults(result);
        setLogs(p => [...p, `\n${result.message}`]);
      } else if (mode === 'generate') {
         // select output folder
         const output = await open({
            directory: true,
            defaultPath: gameFolder,
         });
         
         if (output) {
             const msg = await invoke("generate_goldberg_crack_only", {
               gameFolder,
               outputFolder: output,
               appId: targetAppId,
               config
             });
             setLogs(p => [...p, `\n${msg}`]);
             setGoldbergResults({ success: true, message: msg });
         } else {
             setLoading(false);
             setStep(STEPS.GOLDBERG_CONFIG);
             return;
         }
      } else if (mode === 'restore') {
          const msg = await invoke("restore_goldberg_originals", { gameFolder });
          setLogs(p => [...p, `\n${msg}`]);
          setGoldbergResults({ success: true, message: msg });
      }
    } catch (e) {
      setLogs(p => [...p, `Error: ${e}`]);
    }
  }

  function updateGoldbergOption(key, value) {
    setGoldbergOptions(prev => ({ ...prev, [key]: value }));
  }

  // Goldberg Configuration Component
  const GoldbergConfig = () => (
    <div className="card fade-in" style={{maxWidth: '700px'}}>
      <h2> Goldberg Emulator Config</h2>
      
      {!goldbergAvailable && (
        <div className="warning-box">
          <p>️ Goldberg Emulator files not found! Ensure <code>emu-win-release/release</code> exists.</p>
        </div>
      )}
      
      <div className="section">
        <h3>1. Game Selection</h3>
        <div className="input-group">
          <input 
            placeholder="Select game installation folder"
            value={gameFolder}
            readOnly
          />
          <button className="secondary" style={{width: 'auto'}} onClick={selectGameFolder}>📂</button>
        </div>
        
        <div className="input-group" style={{marginTop: '0.5rem'}}>
             <input 
                 placeholder="Game AppID (Required due to manual mode)"
                 value={appId}
                 onChange={(e) => setAppId(e.target.value)}
                 readOnly={fromDownload && !!appId}
                 style={fromDownload && !!appId ? {opacity: 0.7} : {border: '1px solid var(--primary-color)'}}
             />
             {fromDownload && appId && <span style={{fontSize: '0.8rem', opacity: 0.6, alignSelf: 'center', whiteSpace: 'nowrap'}}> From Download</span>}
        </div>
        <p style={{fontSize: '0.75rem', opacity: 0.5, marginTop: '5px'}}>The AppID is required for the emulator to identify the game.</p>
      </div>
      
      <div className="section">
        <h3>2. Emulator Settings</h3>
        <div className="two-col-grid">
            <div className="form-group">
                <label>Language</label>
                <select 
                    value={goldbergOptions.language}
                    onChange={(e) => updateGoldbergOption('language', e.target.value)}
                >
                    {goldbergLanguages.map(l => <option key={l} value={l}>{l}</option>)}
                </select>
            </div>
            
            <div className="form-group">
                <label>Listen Port</label>
                <input 
                    type="number"
                    value={goldbergOptions.listen_port}
                    onChange={(e) => updateGoldbergOption('listen_port', parseInt(e.target.value))}
                />
            </div>
            
            <div className="form-group">
                <label>Account Name</label>
                <input 
                    value={goldbergOptions.account_name}
                    onChange={(e) => updateGoldbergOption('account_name', e.target.value)}
                />
            </div>
            
            <div className="form-group">
                <label>Steam ID (64-bit)</label>
                <input 
                    value={goldbergOptions.steam_id}
                    onChange={(e) => updateGoldbergOption('steam_id', e.target.value)}
                />
            </div>
        </div>
        
        <div className="options-grid">
            <label className="option-item">
                <input
                  type="checkbox"
                  checked={goldbergOptions.disable_networking}
                  onChange={(e) => updateGoldbergOption('disable_networking', e.target.checked)}
                />
                <div className="option-info">
                  <span className="option-name">Disable Networking</span>
                </div>
            </label>
            
            <label className="option-item">
                <input
                  type="checkbox"
                  checked={goldbergOptions.offline_mode}
                  onChange={(e) => updateGoldbergOption('offline_mode', e.target.checked)}
                />
                <div className="option-info">
                  <span className="option-name">Offline Mode</span>
                </div>
            </label>
            
            <label className="option-item">
                <input
                  type="checkbox"
                  checked={goldbergOptions.enable_overlay}
                  onChange={(e) => updateGoldbergOption('enable_overlay', e.target.checked)}
                />
                <div className="option-info">
                  <span className="option-name">Enable Overlay</span>
                </div>
            </label>
            
             <label className="option-item">
                <input
                  type="checkbox"
                  checked={goldbergOptions.use_experimental}
                  onChange={(e) => updateGoldbergOption('use_experimental', e.target.checked)}
                />
                <div className="option-info">
                  <span className="option-name">Experimental Version</span>
                  <span className="option-desc">Use experimental DLLs</span>
                </div>
            </label>
        </div>
      </div>
      
      <div className="section">
         <h3>3. Advanced</h3>
         <div className="form-group" style={{marginBottom: '0.8rem'}}>
            <label style={{display: 'flex', alignItems: 'center', gap: '0.5rem', cursor: 'pointer'}}>
                <input 
                    type="checkbox" 
                    style={{width: 'auto'}}
                    checked={useCustomSave}
                    onChange={(e) => setUseCustomSave(e.target.checked)}
                />
                Use Custom Save Location
            </label>
            {useCustomSave && (
                <input 
                    placeholder="Path relative to DLL (e.g. steam_settings/saves)"
                    value={goldbergOptions.custom_save_location}
                    onChange={(e) => updateGoldbergOption('custom_save_location', e.target.value)}
                />
            )}
         </div>
         
         <div className="options-grid">
             <label className="option-item">
                <input
                  type="checkbox"
                  checked={goldbergOptions.generate_interfaces}
                  onChange={(e) => updateGoldbergOption('generate_interfaces', e.target.checked)}
                />
                <div className="option-info">
                  <span className="option-name">Generate Steam Interfaces</span>
                  <span className="option-desc">Creates steam_interfaces.txt from original DLL</span>
                </div>
            </label>
         </div>
      </div>
      
      <div className="action-row">
          <button className="primary" onClick={() => runGoldberg('apply')} disabled={!goldbergAvailable}>
             Apply Emulator
          </button>
          <button className="secondary" onClick={() => runGoldberg('restore')} disabled={!goldbergAvailable}>
             Restore Originals
          </button>
      </div>
       <button className="secondary" onClick={() => runGoldberg('generate')} disabled={!goldbergAvailable} style={{marginTop: '0.5rem'}}>
         Generate Crack Only Files...
      </button>

      <div className="spacer"></div>
      <button className="secondary" onClick={() => setStep(fromDownload ? STEPS.MAIN_MENU : STEPS.CRACK_FLOW)}>
        Back
      </button>
    </div>
  );

  // Crack Flow Menu
  const CrackFlowMenu = () => (
    <div className="card fade-in">
      <h2> Crack Game</h2>
      <p style={{opacity: 0.8, marginBottom: '1.5rem'}}>Select a cracking method</p>
      
      <button onClick={() => setStep(STEPS.STEAMLESS_CONFIG)}>
         Remove Steam DRM (Steamless)
      </button>
      <div className="spacer"></div>
      <button onClick={() => setStep(STEPS.GOLDBERG_CONFIG)}>
         Apply Goldberg Emulator
      </button>
      <div className="spacer"></div>
      <button className="secondary" onClick={() => setStep(STEPS.MAIN_MENU)}>Back</button>
    </div>
  );

  const renderContent = () => {
    if (showSearch) {
      return <GameSearch />;
    }

    switch (step) {
      case STEPS.INIT:
        return (
          <div className="card fade-in" style={{background: 'transparent', border: 'none', boxShadow: 'none'}}>
            <div className="loader"></div>
            <p style={{marginTop: '1rem', letterSpacing: '2px', fontWeight: 'bold'}}>ACCESSING SYSTEM...</p>
          </div>
        );
      
      case STEPS.DOTNET_ERROR:
        return (
          <div className="card fade-in">
            <h2 className="text-error">.NET 9 Required</h2>
            <p>This application requires .NET 9.0 Runtime.</p>
            <a href="https://dotnet.microsoft.com/en-us/download/dotnet/thank-you/runtime-9.0.11-windows-x64-installer?cid=getdotnetcore" target="_blank">
              <button>Download .NET 9.0</button>
            </a>
            <div className="spacer"></div>
            <button className="secondary" onClick={checkDotnet}>Retry Check</button>
          </div>
        );

      case STEPS.MAIN_MENU:
        return (
          <div className="card fade-in">
            <div className="ascii-art">
{`
   ____   _    ____ ____     ____  _____ _____  _    __  __ 
  / ___| / \\  / ___|  _ \\   / ___||_   _| ____|/ \\  |  \\/  |
  \\___ \\/ _ \\| |  _| |_) |  \\___ \\  | | |  _| / _ \\ | |\\/| |
   ___) / ___ \\ |_| |  _ <    ___) | | | |___/ ___ \\| |  | |
  |____/_/   \\_\\____|_| \\_\\  |____/  |_| |_____/_/   \\_\\_|  |_|
                                                               
   _____ ___   ___  _     ____  
  |_   _/ _ \\ / _ \\| |   / ___| 
    | || | | | | | | |   \\___ \\ 
    | || |_| | |_| | |___ ___) |
    |_| \\___/ \\___/|_____|____/ 
`}
            </div>
            <h1>ZEUSS STEAM TOOLS</h1>
            <p style={{marginBottom: '2rem', opacity: 0.7}}>[ SYSTEM STATUS: READY ]</p>
            <button onClick={() => setStep(STEPS.DOWNLOAD_METHOD)}> DOWNLOAD CLEAN FILES</button>
            <div className="spacer"></div>
            <button className="secondary" onClick={() => setStep(STEPS.CRACK_FLOW)}> BYPASS DRM / CRACK</button>
          </div>
        );

      case STEPS.DOWNLOAD_METHOD:
        return (
          <div className="card fade-in">
            <h2>Select Method</h2>
            <button onClick={() => setStep(STEPS.MORRENUS_FLOW)}>Morrenus Manifest Manager</button>
            <div className="spacer"></div>
            <button className="secondary" onClick={() => setStep(STEPS.MANUAL_FLOW)}>Manual (Manifest & Lua)</button>
            <div className="spacer"></div>
            <button className="secondary" onClick={() => setStep(STEPS.MAIN_MENU)}>Back</button>
          </div>
        );
        
      case STEPS.MORRENUS_FLOW:
        return (
          <div className="card fade-in">
            <h2>Morrenus Download</h2>
            
            <GameHeader />
            
            <div className="input-group">
              <input 
                placeholder="App ID (e.g. 730)" 
                value={appId} 
                onChange={(e) => setAppId(e.target.value)} 
              />
              <button className="secondary" style={{width: 'auto'}} onClick={() => setShowSearch(true)}>🔍</button>
            </div>
            
            <input 
              placeholder="Morrenus API Key" 
              type="password"
              value={apiKey} 
              onChange={(e) => setApiKey(e.target.value)} 
            />
            
            <div className="input-group">
              <input 
                placeholder="Download Directory"
                value={downloadDir}
                readOnly
              />
              <button className="secondary" style={{width: 'auto', padding: '0 1.2rem'}} onClick={selectFolder}>📂</button>
            </div>
            
            <button onClick={prepareDownload} disabled={preparing}>
              {preparing ? "Preparing..." : "Continue"}
            </button>
            <div className="spacer"></div>
            <button className="secondary" onClick={() => setStep(STEPS.DOWNLOAD_METHOD)}>Back</button>
          </div>
        );
        
      case STEPS.MANUAL_FLOW:
        return (
          <div className="card fade-in">
            <h2>Manual Download</h2>
            
            <GameHeader />
            
            <div className="input-group">
              <input 
                placeholder="App ID (e.g. 730)" 
                value={appId} 
                onChange={(e) => setAppId(e.target.value)} 
              />
              <button className="secondary" style={{width: 'auto'}} onClick={() => setShowSearch(true)}>🔍</button>
            </div>
            
            <div className="input-group">
              <input 
                placeholder="Source (Zip or Folder)"
                value={sourcePath}
                readOnly
              />
              <button className="secondary" style={{width: 'auto'}} onClick={() => selectSource('file')}>📄</button>
              <button className="secondary" style={{width: 'auto'}} onClick={() => selectSource('folder')}>📂</button>
            </div>

            <div className="input-group">
              <input 
                placeholder="Download Directory"
                value={downloadDir}
                readOnly
              />
              <button className="secondary" style={{width: 'auto', padding: '0 1.2rem'}} onClick={selectFolder}>📂</button>
            </div>
            
            <button onClick={prepareDownload} disabled={preparing}>
              {preparing ? "Preparing..." : "Continue"}
            </button>
            <div className="spacer"></div>
            <button className="secondary" onClick={() => setStep(STEPS.DOWNLOAD_METHOD)}>Back</button>
          </div>
        );

      case STEPS.DEPOT_SELECT:
        return <DepotSelection />;

      case STEPS.CRACK_FLOW:
        return <CrackFlowMenu />;

      case STEPS.STEAMLESS_CONFIG:
        return <SteamlessConfig />;

      case STEPS.STEAMLESS_RUNNING:
        return renderConsole("Steamless Output", true);
        
      case STEPS.GOLDBERG_CONFIG:
        return <GoldbergConfig />;
        
      case STEPS.GOLDBERG_RUNNING:
        return renderConsole("Goldberg Output", false);

      default:
        return null;
    }
  };

  const renderConsole = (consoleTitle = "Terminal Output", isSteamless = false) => (
    <div className="console-view fade-in">
      <div className="console-header">
        <span>{consoleTitle}</span>
        <button 
            className="secondary" 
            style={{width: 'auto', fontSize: '0.8rem', padding: '0.4rem 0.8rem'}} 
            onClick={finishProcess}
          >
            {isSteamless ? "Done" : "Close / Back"}
        </button>
      </div>
      <div className="console-logs">
        {logs.map((log, i) => (
          <div key={i} className="log-line">{log}</div>
        ))}
        <div ref={logsEndRef} />
      </div>
      {!isSteamless && (
        <div className="progress-section">
          <div className="progress-info">
            <span style={{maxWidth: '70%', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap'}}>{currentFile}</span>
            <span>{progress.toFixed(1)}%</span>
          </div>
          <div className="progress-bar-bg">
            <div className="progress-fill" style={{width: `${progress}%`}}></div>
          </div>
        </div>
      )}
      {isSteamless && steamlessResults && (
        <div className="results-section">
          <div className="results-summary">
            <div className="result-stat success"> {steamlessResults.successful} Unpacked</div>
            <div className="result-stat warning"> {steamlessResults.results.filter(r => r.message.includes("Not SteamStub")).length} Not Protected</div>
          </div>
          {steamlessResults.successful === 0 && steamlessResults.results.some(r => r.message.includes("Not SteamStub")) && (
            <div className="info-box">
              <p> <strong>Tip:</strong> These executables don't use SteamStub DRM.</p>
            </div>
          )}
        </div>
      )}
      
      {/* Footer Actions */}
      <div className="console-footer-actions">
           {!isSteamless && step !== STEPS.GOLDBERG_RUNNING && progress >= 100 && (
            <button 
              className="primary large" 
              onClick={continueToCrack}
            >
              Continue to Steamless 
            </button>
          )}
          {isSteamless && steamlessResults && (
            <button 
              className="primary large" 
              style={{background: 'var(--success-color)'}} 
              onClick={continueToGoldberg}
            >
              Continue to Goldberg
            </button>
          )}
          {step === STEPS.GOLDBERG_RUNNING && goldbergResults && (
            <>
                <button className="secondary large" onClick={openGameFolderAction}>
                    Open Game Folder
                </button>
                <button className="primary large" onClick={finishProcess}>
                    Done
                </button>
            </>
          )}
      </div>
    </div>
  );

  return (
    <main className="container">
      <div className="scanline"></div>
      {loading && step !== STEPS.STEAMLESS_RUNNING && step !== STEPS.GOLDBERG_RUNNING ? renderConsole() : renderContent()}
    </main>
  );
}

export default App;
