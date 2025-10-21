use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        OnceLock,
    },
    thread,
    time::Duration,
};
use windows::{
    core::{Error as WinError, Result as WinResult},
    Win32::{
        Media::Audio::{
            eConsole, eRender, Endpoints::IAudioEndpointVolume, IMMDeviceEnumerator,
            MMDeviceEnumerator,
        },
        System::Com::{
            CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_APARTMENTTHREADED,
        },
        System::SystemInformation::GetTickCount64,
        UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO},
    },
};

const DEFAULT_INACTIVITY_THRESHOLD_SECS: u64 = 5 * 60;
const POLL_INTERVAL: Duration = Duration::from_secs(5);
const QUIET_VOLUME_LEVEL: f32 = 0.0;
const VOLUME_EPSILON: f32 = 0.02;

static MONITOR: OnceLock<Result<(), String>> = OnceLock::new();
static INACTIVITY_THRESHOLD_SECS: AtomicU64 = AtomicU64::new(DEFAULT_INACTIVITY_THRESHOLD_SECS);

pub fn start_monitor() -> Result<(), String> {
    println!(
        "[sound-inactive] iniciando monitor de inatividade sonora (threshold atual: {} segundos)...",
        inactivity_threshold().as_secs()
    );
    MONITOR
        .get_or_init(|| {
            std::thread::Builder::new()
                .name("sound-inactive-monitor".into())
                .spawn(|| {
                    if let Err(err) = run_monitor() {
                        eprintln!("[sound-inactive] monitor encerrado com erro: {err}");
                    }
                })
                .map(|_| ())
                .map_err(|err| format!("Nao foi possivel iniciar o monitoramento: {err}"))
        })
        .clone()
}

pub fn set_inactivity_threshold(duration: Duration) -> Result<(), String> {
    if duration.is_zero() {
        return Err("O tempo de inatividade deve ser maior que zero.".into());
    }

    let secs = duration.as_secs().max(1);
    INACTIVITY_THRESHOLD_SECS.store(secs, Ordering::Relaxed);
    println!(
        "[sound-inactive] threshold atualizado para {} segundo(s)",
        secs
    );

    Ok(())
}

fn run_monitor() -> Result<(), String> {
    unsafe {
        let _com =
            ComGuard::new().map_err(|err| describe_error("Falha ao inicializar COM", err))?;
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
                .map_err(|err| describe_error("Falha ao criar enumerador de dispositivos", err))?;
        let device = enumerator
            .GetDefaultAudioEndpoint(eRender, eConsole)
            .map_err(|err| describe_error("Falha ao obter dispositivo de audio padrao", err))?;
        let endpoint = device
            .Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None)
            .map_err(|err| describe_error("Falha ao ativar controle de volume", err))?;

        monitor_loop(endpoint).map_err(|err| describe_error("Falha durante monitoramento", err))
    }
}

fn monitor_loop(endpoint: IAudioEndpointVolume) -> WinResult<()> {
    let mut lowered = false;
    let mut previous_volume = 1.0;
    let mut previous_mute_state = false;

    loop {
        let threshold = inactivity_threshold();
        let idle = idle_time()?;

        if idle >= threshold {
            if !lowered {
                let current = current_volume(&endpoint)?;
                let is_muted = unsafe { endpoint.GetMute()?.as_bool() };
                previous_volume = current;
                previous_mute_state = is_muted;

                if !is_muted {
                    unsafe {
                        endpoint.SetMute(true, std::ptr::null())?;
                    }
                }

                if (current - QUIET_VOLUME_LEVEL).abs() > VOLUME_EPSILON {
                    set_volume(&endpoint, QUIET_VOLUME_LEVEL)?;
                }

                lowered = true;
            }
        } else if lowered {
            set_volume(&endpoint, previous_volume)?;

            if !previous_mute_state {
                unsafe {
                    endpoint.SetMute(false, std::ptr::null())?;
                }
            }

            lowered = false;
        }

        thread::sleep(POLL_INTERVAL);
    }
}

fn inactivity_threshold() -> Duration {
    let secs = INACTIVITY_THRESHOLD_SECS.load(Ordering::Relaxed).max(1);
    Duration::from_secs(secs)
}

fn idle_time() -> WinResult<Duration> {
    unsafe {
        let mut info = LASTINPUTINFO {
            cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
            dwTime: 0,
        };

        if !GetLastInputInfo(&mut info).as_bool() {
            return Err(WinError::from_win32());
        }

        let current = GetTickCount64();
        let last_input = u64::from(info.dwTime);
        let idle_ms = current.saturating_sub(last_input);

        Ok(Duration::from_millis(idle_ms))
    }
}

fn current_volume(endpoint: &IAudioEndpointVolume) -> WinResult<f32> {
    unsafe { endpoint.GetMasterVolumeLevelScalar() }
}

fn set_volume(endpoint: &IAudioEndpointVolume, level: f32) -> WinResult<()> {
    let clamped = level.clamp(0.0, 1.0);
    unsafe { endpoint.SetMasterVolumeLevelScalar(clamped, std::ptr::null()) }
}

struct ComGuard {
    should_uninit: bool,
}

impl ComGuard {
    unsafe fn new() -> Result<Self, WinError> {
        let hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

        if hr.is_err() {
            return Err(WinError::from(hr));
        }

        Ok(Self {
            should_uninit: true,
        })
    }
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.should_uninit {
            unsafe {
                CoUninitialize();
            }
        }
    }
}

fn describe_error(context: &str, err: WinError) -> String {
    let message = err.message();

    if message.is_empty() {
        format!("{context}: codigo 0x{:08X}", err.code().0 as u32)
    } else {
        format!("{context}: {message}")
    }
}
