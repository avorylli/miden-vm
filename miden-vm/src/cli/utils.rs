use std::{fs, path::Path, sync::Arc};

use miden_assembly::{
    Assembler, DefaultSourceManager, KernelLibrary, Library,
    diagnostics::{IntoDiagnostic, Report, WrapErr},
};
use miden_mast_package::{MastArtifact, Package};
use miden_prover::utils::Deserializable;

use crate::cli::data::{Debug, Libraries, ProgramFile};

/// Returns a `Program` type from a `.masp` package file.
pub fn get_masp_program(path: &Path) -> Result<miden_core::Program, Report> {
    let bytes = fs::read(path).into_diagnostic().wrap_err("Failed to read package file")?;
    // Use `read_from_bytes` provided by the Deserializable trait.
    let package = Package::read_from_bytes(&bytes)
        .into_diagnostic()
        .wrap_err("Failed to deserialize package")?;
    let program_arc = match package.into_mast_artifact() {
        MastArtifact::Executable(prog_arc) => prog_arc,
        _ => return Err(Report::msg("The provided package is not a program package.")),
    };
    // Unwrap the Arc. If multiple references exist, clone the inner program.
    let program = Arc::try_unwrap(program_arc).unwrap_or_else(|arc| (*arc).clone());
    Ok(program)
}

/// Returns a `Program` type from a `.masm` assembly file.
pub fn get_masm_program(
    path: &Path,
    libraries: &Libraries,
    debug_on: bool,
) -> Result<(miden_core::Program, Arc<DefaultSourceManager>), Report> {
    get_masm_program_with_kernel(path, libraries, debug_on, None)
}

/// Returns a `Program` type from a `.masm` assembly file, optionally with a kernel.
pub fn get_masm_program_with_kernel(
    path: &Path,
    libraries: &Libraries,
    debug_on: bool,
    kernel: Option<&KernelLibrary>,
) -> Result<(miden_core::Program, Arc<DefaultSourceManager>), Report> {
    let debug_mode = if debug_on { Debug::On } else { Debug::Off };
    let program_file = ProgramFile::read(path)?;
    let program = program_file.compile_with_kernel(debug_mode, &libraries.libraries, kernel)?;

    Ok((program, program_file.source_manager().clone()))
}

/// Loads a kernel library from a file path. Supports both .masm and .masl files.
pub fn get_kernel_library(path: &Path, debug_on: bool) -> Result<KernelLibrary, Report> {
    if !path.is_file() {
        return Err(Report::msg(format!("Kernel file `{}` must be a file", path.display())));
    }

    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();

    match ext.as_str() {
        "masl" => {
            // Load from compiled library file
            let library =
                Library::deserialize_from_file(path).into_diagnostic().wrap_err_with(|| {
                    format!("Failed to read kernel library from `{}`", path.display())
                })?;
            KernelLibrary::try_from(library).map_err(|e| {
                Report::msg(format!("Failed to convert library to kernel library: {e}"))
            })
        },
        "masm" => {
            // Compile from source
            let source_manager = Arc::new(DefaultSourceManager::default());
            let mut assembler = Assembler::new(source_manager).with_debug_mode(debug_on);
            assembler
                .link_dynamic_library(miden_stdlib::StdLibrary::default())
                .wrap_err("Failed to load stdlib")?;
            assembler
                .assemble_kernel(path)
                .wrap_err_with(|| format!("Failed to compile kernel from `{}`", path.display()))
        },
        _ => Err(Report::msg(format!(
            "Kernel file must have a .masm or .masl extension, got `{}`",
            path.display()
        ))),
    }
}
