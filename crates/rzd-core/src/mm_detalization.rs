use crate::engine::Engine;
use crate::synthetic_person::SyntheticPersonGenerator;
use chrono::{DateTime, Duration, NaiveDateTime, Utc};
use rust_xlsxwriter::{Format, FormatAlign, Workbook};

/// Экспортирует результаты симуляции в формат "Детализация ММ" (166 колонок, лист "Данные").
pub struct MmDetalizationExporter {
    person_gen: SyntheticPersonGenerator,
}

#[derive(Debug, Clone)]
pub enum MmCell {
    Text(String),
    Number(f64),
    Empty,
    Dash,
}

impl MmCell {
    pub fn as_str(&self) -> String {
        match self {
            MmCell::Text(s) => s.clone(),
            MmCell::Number(n) => n.to_string(),
            MmCell::Empty => String::new(),
            MmCell::Dash => "-".to_string(),
        }
    }
}

pub const MM_HEADERS: &[&str] = &[
    "Эксплуатационное локомотивное депо",
    "Номер маршрута машиниста",
    "Табельный номер (машинист)",
    "ФИО(машинист)",
    "Должностной признак (машинист)",
    "Табельный номер (помощника машиниста)",
    "ФИО(помощника машиниста)",
    "Должностной признак (помощника машиниста)",
    "Табельный номер (3-го лица)",
    "ФИО(3-го лица)",
    "Должностной признак (3-го лица)",
    "Дата и время явки",
    "Дата и время начала приемки ТПС",
    "Дата и время окончания приемки ТПС",
    "Дата и время проследования КП депо при выходе",
    "Дата и время проследования КП депо при заходе",
    "Дата и время начала сдачи ТПС",
    "Дата и время окончания сдачи ТПС",
    "Дата и время окончания работы",
    "1 вид (случай) след.-Дата и время начала следования резервной бригады(пассажиром)",
    "1 вид (случай) след.-Дата и время окончания следования резервной бригады(пассажиром)",
    "1 вид (случай) след.-Начальная станция следования резервной бригады (ЕСР)",
    "1 вид (случай) след.-Название начальной станции следования резервной бригады",
    "1 вид (случай) след.-Конечная станция следования резервной бригады(ЕСР)",
    "1 вид (случай) след.-Название конечной станции следования резервной бригады",
    "2 вид (случай) след.-Дата и время начала следования резервной бригады(пассажиром)",
    "2 вид (случай) след.-Дата и время окончания следования резервной бригады(пассажиром)",
    "2 вид (случай) след.-Начальная станция следования резервной бригады (ЕСР)",
    "2 вид (случай) след.-Название начальной станции следования резервной бригады",
    "2 вид (случай) след.-Конечная станция следования резервной бригады(ЕСР)",
    "2 вид (случай) след.-Название конечной станции следования резервной бригады",
    "1-Серия локомотива в голове",
    "1-Номер локомотива в голове",
    "1-Секция локомотива",
    "1-Депо приписки локомотива",
    "1-Показания счетчика при приемке ТПС",
    "1-Показания счетчика при сдаче ТПС",
    "1-Показания счетчика рекуперации при приемке ТПС",
    "1-Показания счетчика рекуперации при сдаче ТПС",
    "1-Исправен/не исправен счетчик (1-не исправен, 0 - исправен)",
    "2-Серия локомотива в голове",
    "2-Номер локомотива в голове",
    "2-Секция локомотива",
    "2-Депо приписки локомотива",
    "2-Показания счетчика при приемке ТПС",
    "2-Показания счетчика при сдаче ТПС",
    "2-Показания счетчика рекуперации при приемке ТПС",
    "2-Показания счетчика рекуперации при сдаче ТПС",
    "2-Исправен/не исправен счетчик (1-не исправен, 0 - исправен)",
    "3-Серия локомотива в голове",
    "3-Номер локомотива в голове",
    "3-Секция локомотива",
    "3-Депо приписки локомотива",
    "3-Показания счетчика при приемке ТПС",
    "3-Показания счетчика при сдаче ТПС",
    "3-Показания счетчика рекуперации при приемке ТПС",
    "3-Показания счетчика рекуперации при сдаче ТПС",
    "3-Исправен/не исправен счетчик (1-не исправен, 0 - исправен)",
    "4-Серия локомотива в голове",
    "4-Номер локомотива в голове",
    "4-Секция локомотива",
    "4-Депо приписки локомотива",
    "4-Показания счетчика при приемке ТПС",
    "4-Показания счетчика при сдаче ТПС",
    "4-Показания счетчика рекуперации при приемке ТПС",
    "4-Показания счетчика рекуперации при сдаче ТПС",
    "4-Исправен/не исправен счетчик (1-не исправен, 0 - исправен)",
    "5-Серия локомотива в голове",
    "5-Номер локомотива в голове",
    "5-Секция локомотива",
    "5-Депо приписки локомотива",
    "5-Показания счетчика при приемке ТПС",
    "5-Показания счетчика при сдаче ТПС",
    "5-Показания счетчика рекуперации при приемке ТПС",
    "5-Показания счетчика рекуперации при сдаче ТПС",
    "5-Исправен/не исправен счетчик (1-не исправен, 0 - исправен)",
    "6-Серия локомотива в голове",
    "6-Номер локомотива в голове",
    "6-Секция локомотива",
    "6-Депо приписки локомотива",
    "6-Показания счетчика при приемке ТПС",
    "6-Показания счетчика при сдаче ТПС",
    "6-Показания счетчика рекуперации при приемке ТПС",
    "6-Показания счетчика рекуперации при сдаче ТПС",
    "6-Исправен/не исправен счетчик (1-не исправен, 0 - исправен)",
    "7-Серия локомотива в голове",
    "7-Номер локомотива в голове",
    "7-Секция локомотива",
    "7-Депо приписки локомотива",
    "7-Показания счетчика при приемке ТПС",
    "7-Показания счетчика при сдаче ТПС",
    "7-Показания счетчика рекуперации при приемке ТПС",
    "7-Показания счетчика рекуперации при сдаче ТПС",
    "7-Исправен/не исправен счетчик (1-не исправен, 0 - исправен)",
    "8-Серия локомотива в голове",
    "8-Номер локомотива в голове",
    "8-Секция локомотива",
    "8-Депо приписки локомотива",
    "8-Показания счетчика при приемке ТПС",
    "8-Показания счетчика при сдаче ТПС",
    "8-Показания счетчика рекуперации при приемке ТПС",
    "8-Показания счетчика рекуперации при сдаче ТПС",
    "8-Исправен/не исправен счетчик (1-не исправен, 0 - исправен)",
    "9-Серия локомотива в голове",
    "9-Номер локомотива в голове",
    "9-Секция локомотива",
    "9-Депо приписки локомотива",
    "9-Показания счетчика при приемке ТПС",
    "9-Показания счетчика при сдаче ТПС",
    "9-Показания счетчика рекуперации при приемке ТПС",
    "9-Показания счетчика рекуперации при сдаче ТПС",
    "9-Исправен/не исправен счетчик (1-не исправен, 0 - исправен)",
    "10-Серия локомотива в голове",
    "10-Номер локомотива в голове",
    "10-Секция локомотива",
    "10-Депо приписки локомотива",
    "10-Показания счетчика при приемке ТПС",
    "10-Показания счетчика при сдаче ТПС",
    "10-Показания счетчика рекуперации при приемке ТПС",
    "10-Показания счетчика рекуперации при сдаче ТПС",
    "10-Исправен/не исправен счетчик (1-не исправен, 0 - исправен)",
    "11-Серия локомотива в голове",
    "11-Номер локомотива в голове",
    "11-Секция локомотива",
    "11-Депо приписки локомотива",
    "11-Показания счетчика при приемке ТПС",
    "11-Показания счетчика при сдаче ТПС",
    "11-Показания счетчика рекуперации при приемке ТПС",
    "11-Показания счетчика рекуперации при сдаче ТПС",
    "11-Исправен/не исправен счетчик (1-не исправен, 0 - исправен)",
    "12-Серия локомотива в голове",
    "12-Номер локомотива в голове",
    "12-Секция локомотива",
    "12-Депо приписки локомотива",
    "12-Показания счетчика при приемке ТПС",
    "12-Показания счетчика при сдаче ТПС",
    "12-Показания счетчика рекуперации при приемке ТПС",
    "12-Показания счетчика рекуперации при сдаче ТПС",
    "12-Исправен/не исправен счетчик",
    "13-Серия локомотива в голове",
    "13-Номер локомотива в голове",
    "13-Секция локомотива",
    "13-Депо приписки локомотива",
    "13-Показания счетчика при приемке ТПС",
    "13-Показания счетчика при сдаче ТПС",
    "13-Показания счетчика рекуперации при приемке ТПС",
    "13-Показания счетчика рекуперации при сдаче ТПС",
    "13-Исправен/не исправен счетчик (1-не исправен, 0 - исправен)",
    "Начальная станция",
    "Название начальной станции",
    "Дата и время отправления с начальной станции",
    "Номер поезда по отправлению с начальной станции",
    "Масса поезда НЕТТО в тоннах по прибытию на начальную станцию",
    "Масса поезда БРУТТО в тоннах по прибытию на начальную станцию",
    "Конечная станция",
    "Название конечной станции",
    "Дата и время прибытия на конечную станцию",
    "Номер поезда по прибытию на конечную станцию",
    "Масса поезда НЕТТО в тоннах по прибытию на конечную станцию",
    "Масса поезда БРУТТО в тоннах по прибытию на конечную станцию",
    "Количество осей (на нач. станции)",
    "Количество груженых вагонов по отправлению с нач. станции",
    "Количество порожних вагонов по отправлению с нач. станции",
    "Количество осей (на кон. станции)",
    "Количество груженых вагонов по отправлению на кон. станцию",
    "Количество порожних вагонов по отправлению на кон. станцию",
];

impl MmDetalizationExporter {
    pub fn new(seed: u32) -> Self {
        Self {
            person_gen: SyntheticPersonGenerator::new(seed),
        }
    }

    pub fn export_to_xlsx(
        &mut self,
        engine: &Engine,
        output_path: &std::path::Path,
        base_date: &str,
    ) -> Result<(), rust_xlsxwriter::XlsxError> {
        let rows = self.build_rows(engine, base_date);
        self.write_xlsx(&rows, output_path)
    }

    fn build_rows(&mut self, engine: &Engine, base_date: &str) -> Vec<Vec<MmCell>> {
        let naive =
            NaiveDateTime::parse_from_str(&format!("{} 00:00:00", base_date), "%Y-%m-%d %H:%M:%S")
                .unwrap_or_else(|_| {
                    NaiveDateTime::parse_from_str("2026-01-01 00:00:00", "%Y-%m-%d %H:%M:%S")
                        .unwrap()
                });
        let base_dt: DateTime<Utc> = naive.and_utc();

        let mut sorted_tu3: Vec<_> = engine.tu3_rows.iter().collect();
        sorted_tu3.sort_by_key(|r| r.shift_start);

        sorted_tu3
            .iter()
            .enumerate()
            .map(|(i, tu3)| self.build_mm_row(tu3, i as u32 + 1, &base_dt, engine))
            .collect()
    }

    fn build_mm_row(
        &mut self,
        tu3: &crate::tu3::Tu3Row,
        route_number: u32,
        base_dt: &DateTime<Utc>,
        engine: &Engine,
    ) -> Vec<MmCell> {
        let mut row: Vec<MmCell> = Vec::with_capacity(166);

        let depot_code = depot_id_to_code(&tu3.depot_name);
        // Prefer crew entity identity fields (populated by scenario generator)
        // over fresh random generation, ensuring consistency across exports.
        let crew = engine.crews.get(&tu3.crew_id);
        let driver_name = crew
            .filter(|c| !c.full_name.is_empty())
            .map(|c| c.full_name.clone())
            .unwrap_or_else(|| self.person_gen.generate_driver_name());
        let driver_tab = crew
            .filter(|c| c.personnel_number > 0)
            .map(|c| c.personnel_number)
            .unwrap_or_else(|| self.person_gen.generate_personnel_number());
        let assistant_name = crew
            .filter(|c| !c.assistant_full_name.is_empty())
            .map(|c| c.assistant_full_name.clone())
            .unwrap_or_else(|| self.person_gen.generate_assistant_name());
        let assistant_tab = crew
            .filter(|c| c.assistant_personnel_number > 0)
            .map(|c| c.assistant_personnel_number)
            .unwrap_or_else(|| self.person_gen.generate_personnel_number());

        let appear_dt = min_to_dt(base_dt, tu3.appear_time);
        let accept_start_dt = min_to_dt(base_dt, tu3.acceptance_start_time);
        let accept_end_dt = min_to_dt(base_dt, tu3.acceptance_end_time);
        let depart_dt = min_to_dt(base_dt, tu3.departure_time);
        let handover_start_dt = tu3.handover_start_time.map(|t| min_to_dt(base_dt, t));
        let handover_end_dt = tu3.handover_end_time.map(|t| min_to_dt(base_dt, t));
        let work_end_dt = tu3.shift_end.map(|t| min_to_dt(base_dt, t));

        // 1-11: персонал
        row.push(MmCell::Number(depot_code as f64));
        row.push(MmCell::Number(route_number as f64));
        row.push(MmCell::Number(driver_tab as f64));
        row.push(MmCell::Text(driver_name));
        row.push(MmCell::Number(11.0));
        row.push(MmCell::Number(assistant_tab as f64));
        row.push(MmCell::Text(assistant_name));
        row.push(MmCell::Number(21.0));
        row.push(MmCell::Number(0.0));
        row.push(MmCell::Empty);
        row.push(MmCell::Number(0.0));

        // 12-19: временные метки
        row.push(MmCell::Text(fmt_dt(&appear_dt)));
        row.push(MmCell::Text(fmt_dt(&accept_start_dt)));
        row.push(MmCell::Text(fmt_dt(&accept_end_dt)));
        row.push(MmCell::Text(fmt_dt(&depart_dt)));
        row.push(match &handover_start_dt {
            Some(d) => MmCell::Text(fmt_dt(d)),
            None => MmCell::Dash,
        });
        row.push(match &handover_start_dt {
            Some(d) => MmCell::Text(fmt_dt(d)),
            None => MmCell::Dash,
        });
        row.push(match &handover_end_dt {
            Some(d) => MmCell::Text(fmt_dt(d)),
            None => MmCell::Dash,
        });
        row.push(match &work_end_dt {
            Some(d) => MmCell::Text(fmt_dt(d)),
            None => MmCell::Dash,
        });

        // 20-31: следование резервом (нет)
        for _ in 0..12 {
            row.push(MmCell::Dash);
        }

        // 32-40: локомотив 1 (реальный)
        let loco_series = extract_loco_series(&tu3.loco_series);
        let loco_num = self.person_gen.generate_loco_number(500, 3000);
        let counter_base = self.person_gen.generate_counter_reading(5_000_000);
        let counter_delta = (tu3.total_distance_km * 8.0) as u64;
        let regen_base = self.person_gen.generate_counter_reading(500_000);
        let regen_delta = (tu3.total_distance_km * 0.5) as u64;

        row.push(MmCell::Number(loco_series as f64));
        row.push(MmCell::Number(loco_num as f64));
        row.push(MmCell::Number(tu3.loco_sections as f64));
        row.push(MmCell::Number(depot_code as f64));
        row.push(MmCell::Number(counter_base as f64));
        row.push(MmCell::Number((counter_base + counter_delta) as f64));
        row.push(MmCell::Number(regen_base as f64));
        row.push(MmCell::Number((regen_base + regen_delta) as f64));
        row.push(MmCell::Number(0.0));

        // 41-148: локомотивы 2-13 (пустые)
        for _ in 1..13 {
            row.push(MmCell::Number(0.0));
            row.push(MmCell::Number(0.0));
            row.push(MmCell::Number(0.0));
            row.push(MmCell::Number(0.0));
            row.push(MmCell::Number(-1.0));
            row.push(MmCell::Number(-1.0));
            row.push(MmCell::Number(-1.0));
            row.push(MmCell::Number(-1.0));
            row.push(MmCell::Number(-1.0));
        }

        // 149-166: маршрут
        let from_code = station_name_to_code(&tu3.from_station, &engine.network);
        let to_code = station_name_to_code(&tu3.to_station, &engine.network);
        let train_num = 1000 + (self.person_gen.next_u32() % 9000);
        let mass_netto = 500 + (self.person_gen.next_u32() % 3000);
        let mass_brutto = mass_netto + 500 + (self.person_gen.next_u32() % 1000);
        let axles: u32 = 200 + (self.person_gen.next_u32() % 200);
        let loaded: u32 = axles / 4;
        let empty: u32 = (axles / 4).saturating_div(3);

        row.push(MmCell::Number(from_code as f64));
        row.push(MmCell::Text(tu3.from_station.clone()));
        row.push(MmCell::Text(fmt_dt(&depart_dt)));
        row.push(MmCell::Number(train_num as f64));
        row.push(MmCell::Number(mass_netto as f64));
        row.push(MmCell::Number(mass_brutto as f64));
        row.push(MmCell::Number(to_code as f64));
        row.push(MmCell::Text(tu3.to_station.clone()));
        row.push(match &work_end_dt {
            Some(d) => MmCell::Text(fmt_dt(d)),
            None => MmCell::Dash,
        });
        row.push(MmCell::Number(train_num as f64));
        row.push(MmCell::Number(mass_netto as f64));
        row.push(MmCell::Number(mass_brutto as f64));
        row.push(MmCell::Number(axles as f64));
        row.push(MmCell::Number(loaded as f64));
        row.push(MmCell::Number(empty as f64));
        row.push(MmCell::Number(axles as f64));
        row.push(MmCell::Number(loaded as f64));
        row.push(MmCell::Number(empty as f64));

        debug_assert_eq!(row.len(), 166, "MM row must be exactly 166 columns");
        row
    }

    fn write_xlsx(
        &self,
        rows: &[Vec<MmCell>],
        path: &std::path::Path,
    ) -> Result<(), rust_xlsxwriter::XlsxError> {
        let mut workbook = Workbook::new();
        let worksheet = workbook.add_worksheet();
        worksheet.set_name("Данные")?;

        let header_fmt = Format::new()
            .set_bold()
            .set_background_color(rust_xlsxwriter::Color::RGB(0xD9D9D9))
            .set_align(FormatAlign::Center);

        for (col, header) in MM_HEADERS.iter().enumerate() {
            worksheet.write_with_format(0, col as u16, *header, &header_fmt)?;
        }

        for (row_idx, row) in rows.iter().enumerate() {
            let excel_row = (row_idx + 1) as u32;
            for (col_idx, cell) in row.iter().enumerate() {
                let col = col_idx as u16;
                match cell {
                    MmCell::Text(s) => {
                        worksheet.write_string(excel_row, col, s)?;
                    }
                    MmCell::Number(n) => {
                        worksheet.write_number(excel_row, col, *n)?;
                    }
                    MmCell::Empty => {}
                    MmCell::Dash => {
                        worksheet.write_string(excel_row, col, "-")?;
                    }
                }
            }
        }

        worksheet.set_freeze_panes(1, 0)?;
        workbook.save(path)?;
        Ok(())
    }
}

fn min_to_dt(base: &DateTime<Utc>, minutes: u64) -> DateTime<Utc> {
    *base + Duration::minutes(minutes as i64)
}

fn fmt_dt(dt: &DateTime<Utc>) -> String {
    dt.format("%Y-%m-%d %H:%M:%S").to_string()
}

fn depot_id_to_code(depot_id: &str) -> u32 {
    depot_id
        .chars()
        .filter(|c| c.is_ascii_digit())
        .collect::<String>()
        .parse::<u32>()
        .unwrap_or(8801)
}

fn extract_loco_series(series_str: &str) -> u32 {
    series_str
        .chars()
        .filter(|c| c.is_ascii_digit())
        .collect::<String>()
        .parse::<u32>()
        .unwrap_or(233)
}

fn station_name_to_code(name: &str, network: &crate::network::Network) -> u32 {
    for station in network.stations.values() {
        if station.name == name {
            return station
                .id
                .chars()
                .filter(|c| c.is_ascii_digit())
                .collect::<String>()
                .parse::<u32>()
                .unwrap_or(88010);
        }
    }
    88010
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Engine;
    use crate::network::Network;
    use crate::scenario::{generate_scenario, ScenarioConfig};

    fn build_test_engine(seed: u32, days: u32) -> Engine {
        let net_json = include_str!("../../../data/networks/synthetic_one_road_88_05.json");
        let net = Network::from_json(net_json).expect("test network");
        let mut engine = Engine::new(net, seed);
        let config = ScenarioConfig {
            crew_count: 10,
            loco_count: 5,
            seed,
            duration_days: days,
            ..Default::default()
        };
        let gen = generate_scenario(&engine.network, config);
        for c in gen.crews {
            engine.add_crew(c);
        }
        for l in gen.locos {
            engine.add_loco(l);
        }
        engine.schedule_all_shifts(0, 30);
        engine.run((days as u64) * 1440);
        engine
    }

    #[test]
    fn mm_headers_count_is_166() {
        assert_eq!(MM_HEADERS.len(), 166);
    }

    #[test]
    fn mm_row_has_exactly_166_columns() {
        let engine = build_test_engine(42, 2);
        let mut exporter = MmDetalizationExporter::new(42);
        let rows = exporter.build_rows(&engine, "2026-01-01");
        assert!(!rows.is_empty(), "expected at least one TU-3 row");
        for (i, row) in rows.iter().enumerate() {
            assert_eq!(
                row.len(),
                166,
                "row {} has {} columns (expected 166)",
                i,
                row.len()
            );
        }
    }

    #[test]
    fn mm_export_is_deterministic() {
        let engine1 = build_test_engine(77, 2);
        let engine2 = build_test_engine(77, 2);

        let mut exp1 = MmDetalizationExporter::new(77);
        let mut exp2 = MmDetalizationExporter::new(77);

        let rows1 = exp1.build_rows(&engine1, "2026-05-01");
        let rows2 = exp2.build_rows(&engine2, "2026-05-01");

        assert_eq!(rows1.len(), rows2.len());
        for (r1, r2) in rows1.iter().zip(rows2.iter()) {
            for (c1, c2) in r1.iter().zip(r2.iter()) {
                assert_eq!(c1.as_str(), c2.as_str());
            }
        }
    }

    #[test]
    fn driver_name_is_explicitly_synthetic() {
        let mut gen = crate::synthetic_person::SyntheticPersonGenerator::new(1);
        for _ in 0..20 {
            let name = gen.generate_driver_name();
            assert!(
                name.starts_with("СИНТЕТИЧЕСКАЯ БРИГАДА-"),
                "driver label must be explicitly synthetic, got: {}",
                name
            );
        }
    }
}
