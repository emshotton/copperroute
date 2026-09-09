use copper_board::Board;
use copper_dsn::{BoardMetadata, BoardReadResult, CoordinateTransform, DsnReadOptions};
use copper_router::pipeline::prepare_board;
use copper_settings::RouterSettings;

use crate::{Error, FileFormat, RoutingJob};

#[derive(Debug)]
pub struct LoadedBoard {
    pub board: Board,
    pub transform: CoordinateTransform,
    pub metadata: Option<BoardMetadata>,
    pub settings: RouterSettings,
    pub warnings: Vec<String>,
}

pub fn load_from_specctra_dsn(
    bytes: &[u8],
    job: &mut RoutingJob,
    settings: &mut RouterSettings,
) -> Result<LoadedBoard, Error> {
    let ParsedBoard {
        mut board,
        transform,
        metadata,
        warnings,
    } = parse_from_specctra_dsn(bytes, job)?;
    apply_router_settings_for_loaded_board(&mut board, settings);
    apply_immediate_post_load_processing(&mut board);
    Ok(LoadedBoard {
        board,
        transform,
        metadata,
        settings: settings.clone(),
        warnings,
    })
}

pub fn parse_from_specctra_dsn(bytes: &[u8], job: &RoutingJob) -> Result<ParsedBoard, Error> {
    let input_filename = job
        .get_input()
        .map(|input| input.get_filename().to_string())
        .filter(|name| !name.trim().is_empty());
    let result = copper_dsn::read_board(
        bytes,
        None,
        input_filename.as_deref(),
        &DsnReadOptions::default(),
    );
    parse_board_result(result)
}

pub fn load_from_kicad_json(
    text: &str,
    job: &mut RoutingJob,
    settings: &mut RouterSettings,
) -> Result<LoadedBoard, Error> {
    let _ = job;
    let result = kicad_read_board(text);
    apply_parsed_board_result(result, settings)
}

fn kicad_read_board(text: &str) -> BoardReadResult {
    copper_dsn::kicad::read_board(text, None)
}

pub fn load_from_kicad_pcb(
    text: &str,
    name: &str,
    job: &mut RoutingJob,
    settings: &mut RouterSettings,
) -> Result<LoadedBoard, Error> {
    let _ = job;
    let ParsedBoard {
        mut board,
        transform,
        metadata,
        warnings,
    } = parse_from_kicad_pcb(text, name)?;
    apply_router_settings_for_loaded_board(&mut board, settings);
    apply_immediate_post_load_processing(&mut board);
    Ok(LoadedBoard {
        board,
        transform,
        metadata,
        settings: settings.clone(),
        warnings,
    })
}

fn parse_from_kicad_pcb(text: &str, name: &str) -> Result<ParsedBoard, Error> {
    let imported =
        copper_dsn::kicad::read_pcb(text, name, &copper_dsn::kicad::pcb::default_net_class())
            .map_err(|error| Error::Load(error.to_string()))?;
    let mut parsed = parse_board_result(copper_dsn::kicad::read_board_json(imported.board, None))?;
    parsed.warnings.extend(imported.warnings);
    Ok(parsed)
}

pub fn apply_parsed_board_result(
    result: BoardReadResult,
    settings: &mut RouterSettings,
) -> Result<LoadedBoard, Error> {
    let parsed = parse_board_result(result)?;
    let ParsedBoard {
        mut board,
        transform,
        metadata,
        warnings,
    } = parsed;

    apply_router_settings_for_loaded_board(&mut board, settings);
    apply_immediate_post_load_processing(&mut board);

    Ok(LoadedBoard {
        board,
        transform,
        metadata,
        settings: settings.clone(),
        warnings,
    })
}

#[derive(Debug)]
pub struct ParsedBoard {
    pub board: Board,
    pub transform: CoordinateTransform,
    pub metadata: Option<BoardMetadata>,
    pub warnings: Vec<String>,
}

pub fn parse_board_result(result: BoardReadResult) -> Result<ParsedBoard, Error> {
    let (board, metadata, warnings, coordinate_transform) = match result {
        BoardReadResult::Success {
            board,
            metadata,
            warnings,
            coordinate_transform,
        }
        | BoardReadResult::OutlineMissing {
            board,
            metadata,
            warnings,
            coordinate_transform,
        } => (board, metadata, warnings, coordinate_transform),
        BoardReadResult::Partial {
            board,
            metadata,
            mut warnings,
            coordinate_transform,
            diagnostic,
        } => {
            warnings.push(diagnostic);
            (board, metadata, warnings, coordinate_transform)
        }
        BoardReadResult::IoError(error) => {
            return Err(Error::Load(format!(
                "There was an IO error while reading board file.: {error}"
            )));
        }
        BoardReadResult::ParseError { location, detail } => {
            return Err(Error::Load(format!(
                "There was a parse error while reading board file at '{location}': {detail}"
            )));
        }
    };
    let Some(board) = board else {
        return Err(Error::Load(
            "There was a parse error while reading board file at '(pcb': the file produced no board"
                .to_string(),
        ));
    };
    let board = *board;
    let Some(transform) = coordinate_transform else {
        return Err(Error::Load(
            "the reader produced a board without a coordinate transform".to_string(),
        ));
    };

    Ok(ParsedBoard {
        board,
        transform,
        metadata,
        warnings,
    })
}

pub fn apply_router_settings_for_loaded_board(
    board: &mut Board,
    settings: &mut RouterSettings,
) -> bool {
    let board_layer_count = board.get_layer_count();
    if settings.get_layer_count() != board_layer_count {
        settings.set_layer_count(board_layer_count);
    }
    settings.apply_board_specific_optimizations(board);
    prepare_board(board, settings)
}

pub fn apply_immediate_post_load_processing(board: &mut Board) -> bool {
    board.reduce_nets_of_route_items()
}

pub fn load_board_if_needed(job: &mut RoutingJob) -> Result<LoadedBoard, Error> {
    let Some(input) = job.get_input() else {
        return Err(Error::Load(
            "Cannot load board: job has no input".to_string(),
        ));
    };
    let format = input.format;
    if format != FileFormat::Dsn
        && format != FileFormat::KicadDesignJson
        && format != FileFormat::KicadPcb
    {
        return Err(Error::Load(format!(
            "Cannot load board: only DSN, KiCad JSON and KiCad PCB formats are supported, got {}",
            format.name()
        )));
    }
    let data = input.get_data().to_vec();
    let name = input.get_filename_without_extension();
    let mut settings = job.router_settings.clone();
    let loaded = if format == FileFormat::KicadDesignJson {
        let text = String::from_utf8_lossy(&data).into_owned();
        load_from_kicad_json(&text, job, &mut settings)
    } else if format == FileFormat::KicadPcb {
        let text = String::from_utf8_lossy(&data).into_owned();
        load_from_kicad_pcb(&text, &name, job, &mut settings)
    } else {
        load_from_specctra_dsn(&data, job, &mut settings)
    };
    let loaded = loaded.map_err(|error| Error::Load(format!("Failed to load board: {error}")))?;
    job.router_settings = loaded.settings.clone();
    Ok(loaded)
}

pub fn parse_board_if_needed(job: &RoutingJob) -> Result<ParsedBoard, Error> {
    let Some(input) = job.get_input() else {
        return Err(Error::Load(
            "Cannot load board: job has no input".to_string(),
        ));
    };
    let format = input.format;
    if format != FileFormat::Dsn
        && format != FileFormat::KicadDesignJson
        && format != FileFormat::KicadPcb
    {
        return Err(Error::Load(format!(
            "Cannot load board: only DSN, KiCad JSON and KiCad PCB formats are supported, got {}",
            format.name()
        )));
    }
    let data = input.get_data().to_vec();
    let name = input.get_filename_without_extension();
    let parsed = if format == FileFormat::KicadDesignJson {
        parse_board_result(kicad_read_board(&String::from_utf8_lossy(&data)))
    } else if format == FileFormat::KicadPcb {
        parse_from_kicad_pcb(&String::from_utf8_lossy(&data), &name)
    } else {
        parse_from_specctra_dsn(&data, job)
    };
    parsed.map_err(|error| Error::Load(format!("Failed to load board: {error}")))
}
