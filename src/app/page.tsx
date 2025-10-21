"use client";

import { invoke } from "@tauri-apps/api/core";
import { useState, useEffect } from "react";
import { enable, isEnabled, disable } from "@tauri-apps/plugin-autostart";
import {
  Checkbox,
  FormControlLabel,
  Paper,
  TextField,
  Button,
  Stack,
} from "@mui/material";

export default function Home() {
  const [timeoutMinutes, setTimeoutMinutes] = useState<number>(5);
  const [autostartEnabled, setAutostartEnabled] = useState<boolean>(false);

  useEffect(() => {
    loadData();
  }, []);

  async function loadData() {
    const isAutostart = await isEnabled();
    setAutostartEnabled(isAutostart);

    const saved = localStorage.getItem("sound_inactivity_timeout");
    if (saved) {
      const parsed = parseInt(saved, 10);
      if (!isNaN(parsed)) {
        setTimeoutMinutes(parsed);
        invoke("set_sound_inactivity_timeout", {
          minutes: parsed,
        });
      }
    }
  }

  async function setAutoStarted() {
    if (autostartEnabled) {
      await disable();
      setAutostartEnabled(false);
    } else {
      await enable();
      setAutostartEnabled(true);
    }
  }

  async function handleSave() {
    localStorage.setItem("sound_inactivity_timeout", timeoutMinutes.toString());
    try {
      await invoke("set_sound_inactivity_timeout", { minutes: timeoutMinutes });
      alert("Timeout salvo com sucesso!");
    } catch (error) {
      alert("Erro ao salvar timeout: " + error);
    }
  }

  return (
    <Paper className="p-6  flex flex-col h-screen">
      <div className="flex flex-row justify-between items-center w-full">
        <Stack direction="row" spacing={2} alignItems="center">
          <TextField
            id="timeout"
            label="Timeout de Inatividade (minutos)"
            type="number"
            value={timeoutMinutes}
            onChange={(e) =>
              setTimeoutMinutes(parseInt(e.target.value, 10) || 0)
            }
            size="small"
          />
          <Button variant="contained" color="primary" onClick={handleSave}>
            Salvar
          </Button>
        </Stack>
        <FormControlLabel
          control={
            <Checkbox
              checked={autostartEnabled}
              onChange={() => setAutoStarted()}
              color="primary"
            />
          }
          label="Iniciar com o sistema"
        />
      </div>
    </Paper>
  );
}
