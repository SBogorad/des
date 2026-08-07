use crate::prng::Mulberry32;

/// Явно синтетические идентификаторы персонала.
/// Они не имитируют ФИО и не должны совпадать с реальными людьми.
const CREW_CALLSIGNS: &[&str] = &[
    "Альфа",
    "Бета",
    "Гамма",
    "Дельта",
    "Эхо",
    "Зета",
    "Иота",
    "Каппа",
    "Лямбда",
    "Мю",
    "Ню",
    "Омега",
    "Пси",
    "Ро",
    "Сигма",
    "Тау",
    "Фи",
    "Хи",
    "Ипсилон",
    "Йота",
    "Кси",
    "Эпсилон",
    "Эта",
    "Тета",
];

/// Генератор синтетических идентификаторов персонала.
/// Использует Mulberry32 PRNG для детерминированности.
pub struct SyntheticPersonGenerator {
    prng: Mulberry32,
    next_personnel_number: u32,
}

impl SyntheticPersonGenerator {
    pub fn new(seed: u32) -> Self {
        Self {
            prng: Mulberry32::new(seed),
            next_personnel_number: 10000,
        }
    }

    /// Генерирует явно синтетический позывной машиниста.
    pub fn generate_driver_name(&mut self) -> String {
        let callsign = CREW_CALLSIGNS[self.prng.next_u32() as usize % CREW_CALLSIGNS.len()];
        format!(
            "СИНТЕТИЧЕСКАЯ БРИГАДА-{}-{:04}",
            callsign,
            self.prng.next_u32() % 10_000
        )
    }

    /// Генерирует явно синтетический позывной помощника.
    pub fn generate_assistant_name(&mut self) -> String {
        let callsign = CREW_CALLSIGNS[self.prng.next_u32() as usize % CREW_CALLSIGNS.len()];
        format!(
            "СИНТЕТИЧЕСКИЙ ПОМОЩНИК-{}-{:04}",
            callsign,
            self.prng.next_u32() % 10_000
        )
    }

    /// Генерирует уникальный синтетический табельный номер.
    pub fn generate_personnel_number(&mut self) -> u32 {
        let n = self.next_personnel_number;
        self.next_personnel_number += self.prng.next_u32() % 50 + 1;
        n
    }

    /// Генерирует синтетический номер локомотива (до 5 цифр)
    pub fn generate_loco_number(&mut self, min: u32, max: u32) -> u32 {
        min + self.prng.next_u32() % (max - min + 1)
    }

    /// Генерирует синтетическое показание счётчика электроэнергии.
    pub fn generate_counter_reading(&mut self, base: u64) -> u64 {
        base + (self.prng.next_u32() as u64) % 5_000_000
    }

    /// Следующее случайное значение (открытый доступ для MM-экспортёра)
    pub fn next_u32(&mut self) -> u32 {
        self.prng.next_u32()
    }
}
