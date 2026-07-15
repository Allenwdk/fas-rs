import {
  useState,
  useEffect,
  createContext,
  type Dispatch,
  type SetStateAction,
  useCallback,
} from "react";
import { useDebouncedCallback } from "use-debounce";
import {
  ConfigOptions,
  GameList,
  PowerModes,
  FpsValue,
  PowerSettings,
  FpsLockList,
} from "@/types/config";
import { toast } from "sonner";
import { exec } from "../lib/kernelsu";
import TOML from "@iarna/toml";

const defaultConfig: ConfigOptions = {
  keep_std: true,
  scene_game_list: true,
  language: "en",
};

const defaultGameList: GameList = {};

const defaultFpsLock: FpsLockList = {};

const defaultPowerModes: PowerModes = {
  powersave: {
    margin_fps: 3.0,
    core_temp_thresh: 80000 as number | "disabled",
  },
  balance: {
    margin_fps: 1.0,
    core_temp_thresh: 90000 as number | "disabled",
  },
  performance: {
    margin_fps: 0.3,
    core_temp_thresh: 95000 as number | "disabled",
  },
  fast: {
    margin_fps: 0,
    core_temp_thresh: 95000 as number | "disabled",
  },
};

interface ConfigContextType {
  configOptions: ConfigOptions;
  gameList: GameList;
  powerModes: PowerModes;
  fpsLock: FpsLockList;
  newGamePackage: string;
  setNewGamePackage: Dispatch<SetStateAction<string>>;
  newGameFps: string;
  setNewGameFps: Dispatch<SetStateAction<string>>;
  isAddingGame: boolean;
  setIsAddingGame: Dispatch<SetStateAction<boolean>>;
  editingGame: string | null;
  editingGameFps: string;
  setEditingGameFps: Dispatch<SetStateAction<string>>;
  newFpsLockPackage: string;
  setNewFpsLockPackage: Dispatch<SetStateAction<string>>;
  newFpsLockMin: string;
  setNewFpsLockMin: Dispatch<SetStateAction<string>>;
  newFpsLockMax: string;
  setNewFpsLockMax: Dispatch<SetStateAction<string>>;
  isAddingFpsLock: boolean;
  setIsAddingFpsLock: Dispatch<SetStateAction<boolean>>;
  editingFpsLock: string | null;
  editingFpsLockMin: string;
  setEditingFpsLockMin: Dispatch<SetStateAction<string>>;
  editingFpsLockMax: string;
  setEditingFpsLockMax: Dispatch<SetStateAction<string>>;
  toggleConfigOption: (option: keyof ConfigOptions) => void;
  updatePowerMode: (
    mode: keyof PowerModes,
    setting: keyof PowerSettings,
    value: number | number[] | "disabled",
  ) => void;
  addNewGame: () => void;
  removeGame: (gamePackage: string) => void;
  startEditGame: (game: string, fps: FpsValue) => void;
  saveEditedGame: () => void;
  addNewFpsLock: () => void;
  removeFpsLock: (gamePackage: string) => void;
  startEditFpsLock: (game: string, fpsLock: [number, number]) => void;
  saveEditedFpsLock: () => void;
  saveConfiguration: () => void;
  toggleLanguage: () => void;
  language: "en" | "zh";
}

export const ConfigContext = createContext<ConfigContextType>(
  {} as ConfigContextType,
);

export function useConfig() {
  const [configOptions, setConfigOptions] =
    useState<ConfigOptions>(defaultConfig);
  const [gameList, setGameList] = useState<GameList>(defaultGameList);
  const [powerModes, setPowerModes] = useState<PowerModes>(defaultPowerModes);
  const [fpsLock, setFpsLock] = useState<FpsLockList>(defaultFpsLock);
  const [language, setLanguage] = useState<"en" | "zh">("en");

  useEffect(() => {
    const initializeConfig = async () => {
      try {
        const configData = await readConfig();
        setConfigOptions(configData.configOptions || defaultConfig);
        setGameList(configData.gameList || defaultGameList);
        setPowerModes(configData.powerModes || defaultPowerModes);
        setFpsLock(configData.fpsLock || defaultFpsLock);
      } catch (_error) {
        toast.error("Failed to load configuration");
      }
    };

    const savedLang = localStorage.getItem("fasrs-lang");
    if (savedLang === "zh") {
      setLanguage("zh");
    }

    initializeConfig();
  }, []);

  const toggleLanguage = () => {
    const newLang = language === "en" ? "zh" : "en";
    setLanguage(newLang);
    if (typeof window !== "undefined") {
      localStorage.setItem("fasrs-lang", newLang);
      window.location.reload();
    }
  };

  const [newGamePackage, setNewGamePackage] = useState<string>("");
  const [newGameFps, setNewGameFps] = useState<string>("");
  const [isAddingGame, setIsAddingGame] = useState<boolean>(false);
  const [editingGame, setEditingGame] = useState<string | null>(null);
  const [editingGameFps, setEditingGameFps] = useState<string>("");

  const [newFpsLockPackage, setNewFpsLockPackage] = useState<string>("");
  const [newFpsLockMin, setNewFpsLockMin] = useState<string>("");
  const [newFpsLockMax, setNewFpsLockMax] = useState<string>("");
  const [isAddingFpsLock, setIsAddingFpsLock] = useState<boolean>(false);
  const [editingFpsLock, setEditingFpsLock] = useState<string | null>(null);
  const [editingFpsLockMin, setEditingFpsLockMin] = useState<string>("");
  const [editingFpsLockMax, setEditingFpsLockMax] = useState<string>("");

  const debouncedSave = useDebouncedCallback(() => {
    saveConfiguration();
  }, 300);

  const toggleConfigOption = (option: keyof ConfigOptions): void => {
    setConfigOptions((prev) => {
      return {
        ...prev,
        [option]: !prev[option],
      };
    });
    debouncedSave();
  };

  const updatePowerMode = (
    mode: keyof PowerModes,
    setting: keyof PowerSettings,
    value: number | number[] | "disabled",
  ): void => {
    setPowerModes((prev) => {
      return {
        ...prev,
        [mode]: {
          ...prev[mode],
          [setting]:
            setting === "core_temp_thresh" && value === "disabled"
              ? "disabled"
              : setting === "margin_fps"
                ? typeof value === "number"
                  ? value
                  : value[0]
                : typeof value === "number"
                  ? Math.round(value)
                  : Math.round(value[0] as number),
        },
      };
    });
    debouncedSave();
  };

  const addNewGame = (): void => {
    if (newGamePackage && newGameFps) {
      const fpsValues = newGameFps
        .split(",")
        .map((v) => parseInt(v.trim()))
        .filter((v) => !isNaN(v));
      const fpsValue: FpsValue =
        fpsValues.length === 1 ? fpsValues[0] : fpsValues;

      setGameList({
        ...gameList,
        [newGamePackage]: fpsValue,
      });

      setNewGamePackage("");
      setNewGameFps("");
      setIsAddingGame(false);
      toast.success("Game added successfully!");
      debouncedSave();
    }
  };

  const removeGame = (gamePackage: string): void => {
    const updatedGameList = { ...gameList };
    delete updatedGameList[gamePackage];
    setGameList(updatedGameList);
    toast.success("Game removed successfully!");
    debouncedSave();
  };

  const startEditGame = (game: string, fps: FpsValue): void => {
    setEditingGame(game);
    setEditingGameFps(Array.isArray(fps) ? fps.join(", ") : fps.toString());
  };

  const saveEditedGame = (): void => {
    if (editingGame && editingGameFps) {
      let fpsValue: FpsValue;
      if (editingGameFps.includes(",")) {
        fpsValue = editingGameFps
          .split(",")
          .map((v) => parseInt(v.trim()))
          .filter((v) => !isNaN(v));
      } else {
        fpsValue = parseInt(editingGameFps.trim());
      }

      setGameList({
        ...gameList,
        [editingGame]: fpsValue,
      });

      setEditingGame(null);
      setEditingGameFps("");
      toast.success("Game settings updated!");
      debouncedSave();
    }
  };

  const addNewFpsLock = (): void => {
    if (newFpsLockPackage && newFpsLockMin && newFpsLockMax) {
      const min = parseInt(newFpsLockMin.trim());
      const max = parseInt(newFpsLockMax.trim());
      if (isNaN(min) || isNaN(max)) {
        toast.error("Invalid FPS values");
        return;
      }
      const sorted: [number, number] = min <= max ? [min, max] : [max, min];

      setFpsLock({
        ...fpsLock,
        [newFpsLockPackage]: sorted,
      });

      setNewFpsLockPackage("");
      setNewFpsLockMin("");
      setNewFpsLockMax("");
      setIsAddingFpsLock(false);
      toast.success("Fps lock added successfully!");
      debouncedSave();
    }
  };

  const removeFpsLock = (gamePackage: string): void => {
    const updatedFpsLock = { ...fpsLock };
    delete updatedFpsLock[gamePackage];
    setFpsLock(updatedFpsLock);
    toast.success("Fps lock removed successfully!");
    debouncedSave();
  };

  const startEditFpsLock = (
    game: string,
    lock: [number, number],
  ): void => {
    if (game === "") {
      setEditingFpsLock(null);
      return;
    }
    setEditingFpsLock(game);
    setEditingFpsLockMin(lock[0].toString());
    setEditingFpsLockMax(lock[1].toString());
  };

  const saveEditedFpsLock = (): void => {
    if (editingFpsLock && editingFpsLockMin && editingFpsLockMax) {
      const min = parseInt(editingFpsLockMin.trim());
      const max = parseInt(editingFpsLockMax.trim());
      if (isNaN(min) || isNaN(max)) {
        toast.error("Invalid FPS values");
        return;
      }
      const sorted: [number, number] = min <= max ? [min, max] : [max, min];

      setFpsLock({
        ...fpsLock,
        [editingFpsLock]: sorted,
      });

      setEditingFpsLock(null);
      setEditingFpsLockMin("");
      setEditingFpsLockMax("");
      toast.success("Fps lock updated!");
      debouncedSave();
    }
  };

  useEffect(() => {
    return () => {
      debouncedSave.cancel();
    };
  }, [debouncedSave]);

  const saveConfiguration = useCallback(async (): Promise<void> => {
    try {
      await writeConfig({
        configOptions,
        gameList,
        powerModes,
        fpsLock,
      });
      toast.success("Configuration saved successfully!");
    } catch (error) {
      toast.error("Failed to save configuration: " + error);
    }
  }, [configOptions, gameList, powerModes, fpsLock]);

  const readConfig = async (): Promise<{
    configOptions: ConfigOptions;
    gameList: GameList;
    powerModes: PowerModes;
    fpsLock: FpsLockList;
  }> => {
    if (process.env.NODE_ENV === "development") {
      return {
        configOptions: {
          keep_std: true,
          scene_game_list: true,
          language: "en",
        },
        gameList: {
          "com.example.game1": 60,
          "com.example.game2": [45, 60],
        },
        powerModes: {
          powersave: { margin_fps: 3.0, core_temp_thresh: 80000 },
          balance: { margin_fps: 1.0, core_temp_thresh: 90000 },
          performance: { margin_fps: 0.3, core_temp_thresh: 95000 },
          fast: { margin_fps: 0, core_temp_thresh: 95000 },
        },
        fpsLock: {
          "com.example.game3": [60, 120],
        },
      };
    }

    const { errno, stdout, stderr } = await exec(
      `cat /sdcard/Android/fas-rs/games.toml`,
      { cwd: "/" },
    );

    if (errno !== 0) {
      if (stderr.includes("No such file")) {
        toast.error("Configuration file not found, please create it first");
        throw new Error("Configuration file not found");
      }
      throw new Error(`Read config failed: ${stderr}`);
    }

    const configRaw = TOML.parse(stdout) as {
      config: ConfigOptions;
      game_list: GameList;
      fps_lock?: FpsLockList;
      powersave: PowerSettings;
      balance: PowerSettings;
      performance: PowerSettings;
      fast: PowerSettings;
    };

    return {
      configOptions: configRaw.config,
      gameList: configRaw.game_list,
      fpsLock: configRaw.fps_lock || defaultFpsLock,
      powerModes: {
        powersave: configRaw.powersave,
        balance: configRaw.balance,
        performance: configRaw.performance,
        fast: configRaw.fast,
      },
    };
  };

  const writeConfig = async (data: {
    configOptions: ConfigOptions;
    gameList: GameList;
    powerModes: PowerModes;
    fpsLock: FpsLockList;
  }): Promise<void> => {
    if (process.env.NODE_ENV === "development") {
      console.log("Development mode: Skipping actual config write");
      return;
    }

    try {
      const tomlContent = TOML.stringify({
        config: data.configOptions,
        game_list: data.gameList,
        ...(Object.keys(data.fpsLock).length > 0
          ? { fps_lock: data.fpsLock }
          : {}),
        powersave: data.powerModes.powersave,
        balance: data.powerModes.balance,
        performance: data.powerModes.performance,
        fast: data.powerModes.fast,
      })
        .replace(/\[\s+/g, "[")
        .replace(/\s+\]/g, "]");

      const mkdirResult = await exec(`mkdir -p /sdcard/Android/fas-rs`, {
        cwd: "/",
      });
      if (mkdirResult.errno !== 0) {
        throw new Error(`Create directory failed: ${mkdirResult.stderr}`);
      }

      const { errno, stderr } = await exec(
        `echo '${tomlContent.replace(/'/g, "'\\''")}' > /sdcard/Android/fas-rs/games.toml`,
        { cwd: "/" },
      );

      if (errno !== 0) {
        throw new Error(`Write config failed: ${stderr}`);
      }
    } catch (error) {
      toast.error("Failed to save configuration");
      throw error;
    }
  };

  return {
    configOptions,
    gameList,
    powerModes,
    fpsLock,
    newGamePackage,
    setNewGamePackage,
    newGameFps,
    setNewGameFps,
    isAddingGame,
    setIsAddingGame,
    editingGame,
    editingGameFps,
    setEditingGameFps,
    newFpsLockPackage,
    setNewFpsLockPackage,
    newFpsLockMin,
    setNewFpsLockMin,
    newFpsLockMax,
    setNewFpsLockMax,
    isAddingFpsLock,
    setIsAddingFpsLock,
    editingFpsLock,
    editingFpsLockMin,
    setEditingFpsLockMin,
    editingFpsLockMax,
    setEditingFpsLockMax,
    toggleConfigOption,
    updatePowerMode,
    addNewGame,
    removeGame,
    startEditGame,
    saveEditedGame,
    addNewFpsLock,
    removeFpsLock,
    startEditFpsLock,
    saveEditedFpsLock,
    toggleLanguage,
    language,
  };
}
