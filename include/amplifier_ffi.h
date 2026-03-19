#ifndef AMPLIFIER_FFI_H
#define AMPLIFIER_FFI_H

#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

/**
 * FFI result code type.
 */
typedef int32_t AmplifierResult;

/**
 * Opaque handle to a Rust-managed object.
 */
typedef void *AmplifierHandle;

/**
 * Operation succeeded.
 */
#define AMPLIFIER_OK 0

/**
 * A null handle was passed where a valid handle was required.
 */
#define ERR_NULL_HANDLE -1

/**
 * The provided JSON string was invalid or could not be parsed.
 */
#define ERR_INVALID_JSON -2

/**
 * A Tokio runtime error occurred.
 */
#define ERR_RUNTIME -3

/**
 * A session-level error occurred.
 */
#define ERR_SESSION -4

/**
 * An unexpected internal error occurred.
 */
#define ERR_INTERNAL -99

/**
 * Return a pointer to the last error message set on the current thread.
 *
 * The pointer is valid until the next FFI call on this thread that may set
 * a new error.  The caller must **not** free this pointer.  Returns null if
 * no error has been recorded.
 */
const char *amplifier_last_error(void);

/**
 * Free a `*mut c_char` that was allocated by any `amplifier_*` function.
 *
 * Null-safe: calling with a null pointer is a no-op.
 */
void amplifier_string_free(char *ptr);

/**
 * Create a new multi-thread Tokio runtime.
 *
 * On success writes a non-null handle into `*out` and returns `AMPLIFIER_OK`.
 * Returns `ERR_NULL_HANDLE` if `out` is null.
 * Returns `ERR_RUNTIME` if the runtime cannot be created.
 */
AmplifierResult amplifier_runtime_create(AmplifierHandle *out);

/**
 * Destroy a runtime handle created by `amplifier_runtime_create`.
 *
 * Consumes the Arc, which drops the Tokio runtime and shuts it down.
 * Returns `ERR_NULL_HANDLE` if `runtime` is null.
 */
AmplifierResult amplifier_runtime_destroy(AmplifierHandle runtime);

/**
 * Create a new session from a runtime handle and a JSON configuration string.
 *
 * Parses `config_json` as a `SessionConfig`, creates a new `Session`, and
 * writes the session handle to `*out`.
 *
 * Returns:
 * - `AMPLIFIER_OK` on success.
 * - `ERR_NULL_HANDLE` if any pointer argument is null.
 * - `ERR_INVALID_JSON` if `config_json` is not valid UTF-8 or valid session JSON.
 */
AmplifierResult amplifier_session_create(AmplifierHandle runtime,
                                         const char *config_json,
                                         AmplifierHandle *out);

/**
 * Destroy a session handle created by `amplifier_session_create`.
 *
 * Consumes the Arc, dropping the session when the reference count reaches zero.
 * Returns `ERR_NULL_HANDLE` if `session` is null.
 */
AmplifierResult amplifier_session_destroy(AmplifierHandle session);

/**
 * Mark the session as initialized and ready for execution.
 *
 * Acquires the session mutex and calls `set_initialized()` on the inner session.
 *
 * Returns:
 * - `AMPLIFIER_OK` on success.
 * - `ERR_NULL_HANDLE` if `session` is null.
 * - `ERR_SESSION` if the mutex is poisoned.
 */
AmplifierResult amplifier_session_initialize(AmplifierHandle session);

/**
 * Execute a prompt using the session's mounted orchestrator.
 *
 * Blocks the calling thread until the async execution completes.
 * On success, writes a heap-allocated C string pointer to `*out_json`.
 * The caller must free the returned string with `amplifier_string_free`.
 *
 * Returns:
 * - `AMPLIFIER_OK` on success.
 * - `ERR_NULL_HANDLE` if `session`, `prompt`, or `out_json` is null.
 * - `ERR_INVALID_JSON` if `prompt` is not valid UTF-8.
 * - `ERR_SESSION` if execution fails or the mutex is poisoned.
 */
AmplifierResult amplifier_session_execute(AmplifierHandle session,
                                          const char *prompt,
                                          char **out_json);

/**
 * Run session cleanup, emitting `session:end` and releasing resources.
 *
 * Blocks the calling thread until the async cleanup completes.
 *
 * Returns:
 * - `AMPLIFIER_OK` on success.
 * - `ERR_NULL_HANDLE` if `session` is null.
 * - `ERR_SESSION` if the mutex is poisoned.
 */
AmplifierResult amplifier_session_cleanup(AmplifierHandle session);

/**
 * Mount a provider onto a session by name.
 *
 * Validates all arguments: `session`, `provider`, and `name` must be
 * non-null, and `name` must be valid UTF-8.
 *
 * # Returns
 *
 * - `ERR_NULL_HANDLE` if any argument is null.
 * - `ERR_INTERNAL` after argument validation (TODO: kernel integration).
 *
 * # TODO
 *
 * Once `Session`/`Coordinator` expose a public `mount_provider` that accepts
 * an FFI-wrapped `Arc<dyn Provider>`, replace the `ERR_INTERNAL` return with:
 *   1. `handle_to_arc_ref::<FfiSession>(session)` to borrow the session.
 *   2. Lock `session_arc.session`.
 *   3. Access the `Coordinator` via `session.coordinator()`.
 *   4. Call `coordinator.mount_provider(name_str, provider_arc)`.
 */
AmplifierResult amplifier_session_mount_provider(AmplifierHandle session,
                                                 AmplifierHandle provider,
                                                 const char *name);

/**
 * Mount a tool onto a session by name.
 *
 * Validates all arguments: `session`, `tool`, and `name` must be
 * non-null, and `name` must be valid UTF-8.
 *
 * # Returns
 *
 * - `ERR_NULL_HANDLE` if any argument is null.
 * - `ERR_INTERNAL` after argument validation (TODO: kernel integration).
 *
 * # TODO
 *
 * Once `Session`/`Coordinator` expose a public `mount_tool` that accepts
 * an FFI-wrapped `Arc<dyn Tool>`, replace the `ERR_INTERNAL` return with:
 *   1. `handle_to_arc_ref::<FfiSession>(session)` to borrow the session.
 *   2. Lock `session_arc.session`.
 *   3. Access the `Coordinator` via `session.coordinator()`.
 *   4. Call `coordinator.mount_tool(name_str, tool_arc)`.
 */
AmplifierResult amplifier_session_mount_tool(AmplifierHandle session,
                                             AmplifierHandle tool,
                                             const char *name);

/**
 * Set the orchestrator module on a session (single slot).
 *
 * Validates `session` and `orchestrator` are non-null.
 *
 * # Returns
 *
 * - `ERR_NULL_HANDLE` if any argument is null.
 * - `ERR_INTERNAL` after argument validation (TODO: kernel integration).
 *
 * # TODO
 *
 * Once `Session`/`Coordinator` expose a public `set_orchestrator` that accepts
 * an FFI-wrapped `Arc<dyn Orchestrator>`, replace the `ERR_INTERNAL` return with:
 *   1. `handle_to_arc_ref::<FfiSession>(session)` to borrow the session.
 *   2. Lock `session_arc.session`.
 *   3. Access the `Coordinator` via `session.coordinator()`.
 *   4. Call `coordinator.set_orchestrator(orchestrator_arc)`.
 */
AmplifierResult amplifier_session_set_orchestrator(AmplifierHandle session,
                                                   AmplifierHandle orchestrator);

/**
 * Set the context manager module on a session (single slot).
 *
 * Validates `session` and `context` are non-null.
 *
 * # Returns
 *
 * - `ERR_NULL_HANDLE` if any argument is null.
 * - `ERR_INTERNAL` after argument validation (TODO: kernel integration).
 *
 * # TODO
 *
 * Once `Session`/`Coordinator` expose a public `set_context` that accepts
 * an FFI-wrapped `Arc<dyn ContextManager>`, replace the `ERR_INTERNAL` return with:
 *   1. `handle_to_arc_ref::<FfiSession>(session)` to borrow the session.
 *   2. Lock `session_arc.session`.
 *   3. Access the `Coordinator` via `session.coordinator()`.
 *   4. Call `coordinator.set_context(context_arc)`.
 */
AmplifierResult amplifier_session_set_context(AmplifierHandle session, AmplifierHandle context);

/**
 * Load a gRPC-backed provider module from the given endpoint.
 *
 * Validates that `runtime`, `endpoint`, and `out` are all non-null, and that
 * `endpoint` is valid UTF-8.
 *
 * # Returns
 *
 * - `ERR_NULL_HANDLE` if any argument is null.
 * - `ERR_INTERNAL` after argument validation (TODO: gRPC transport integration).
 *
 * # TODO
 *
 * Once a gRPC provider transport adapter is available, replace the
 * `ERR_INTERNAL` return with:
 *   1. Parse `endpoint` into a connection URI.
 *   2. Establish a gRPC channel using the runtime's Tokio executor.
 *   3. Construct a `dyn Provider` from the remote gRPC service.
 *   4. Write the resulting Arc handle into `*out`.
 */
AmplifierResult amplifier_load_grpc_provider(AmplifierHandle runtime,
                                             const char *endpoint,
                                             AmplifierHandle *out);

/**
 * Load a gRPC-backed tool module from the given endpoint.
 *
 * Validates that `runtime`, `endpoint`, and `out` are all non-null, and that
 * `endpoint` is valid UTF-8.
 *
 * # Returns
 *
 * - `ERR_NULL_HANDLE` if any argument is null.
 * - `ERR_INTERNAL` after argument validation (TODO: gRPC transport integration).
 *
 * # TODO
 *
 * Once a gRPC tool transport adapter is available, replace the
 * `ERR_INTERNAL` return with:
 *   1. Parse `endpoint` into a connection URI.
 *   2. Establish a gRPC channel using the runtime's Tokio executor.
 *   3. Construct a `dyn Tool` from the remote gRPC service.
 *   4. Write the resulting Arc handle into `*out`.
 */
AmplifierResult amplifier_load_grpc_tool(AmplifierHandle runtime,
                                         const char *endpoint,
                                         AmplifierHandle *out);

/**
 * Load a gRPC-backed orchestrator module from the given endpoint.
 *
 * Validates that `runtime`, `endpoint`, `session_id`, and `out` are all
 * non-null, and that `endpoint` and `session_id` are valid UTF-8.
 *
 * # Returns
 *
 * - `ERR_NULL_HANDLE` if any argument is null.
 * - `ERR_INTERNAL` after argument validation (TODO: gRPC transport integration).
 *
 * # TODO
 *
 * Once a gRPC orchestrator transport adapter is available, replace the
 * `ERR_INTERNAL` return with:
 *   1. Parse `endpoint` and `session_id` into connection parameters.
 *   2. Establish a gRPC channel using the runtime's Tokio executor.
 *   3. Construct a `dyn Orchestrator` from the remote gRPC service.
 *   4. Write the resulting Arc handle into `*out`.
 */
AmplifierResult amplifier_load_grpc_orchestrator(AmplifierHandle runtime,
                                                 const char *endpoint,
                                                 const char *session_id,
                                                 AmplifierHandle *out);

/**
 * Load a gRPC-backed hook module from the given endpoint.
 *
 * Validates that `runtime`, `endpoint`, and `out` are all non-null, and that
 * `endpoint` is valid UTF-8.
 *
 * # Returns
 *
 * - `ERR_NULL_HANDLE` if any argument is null.
 * - `ERR_INTERNAL` after argument validation (TODO: gRPC transport integration).
 *
 * # TODO
 *
 * Once a gRPC hook transport adapter is available, replace the
 * `ERR_INTERNAL` return with:
 *   1. Parse `endpoint` into a connection URI.
 *   2. Establish a gRPC channel using the runtime's Tokio executor.
 *   3. Construct a `dyn Hook` from the remote gRPC service.
 *   4. Write the resulting Arc handle into `*out`.
 */
AmplifierResult amplifier_load_grpc_hook(AmplifierHandle runtime,
                                         const char *endpoint,
                                         AmplifierHandle *out);

/**
 * Load a gRPC-backed context module from the given endpoint.
 *
 * Validates that `runtime`, `endpoint`, and `out` are all non-null, and that
 * `endpoint` is valid UTF-8.
 *
 * # Returns
 *
 * - `ERR_NULL_HANDLE` if any argument is null.
 * - `ERR_INTERNAL` after argument validation (TODO: gRPC transport integration).
 *
 * # TODO
 *
 * Once a gRPC context transport adapter is available, replace the
 * `ERR_INTERNAL` return with:
 *   1. Parse `endpoint` into a connection URI.
 *   2. Establish a gRPC channel using the runtime's Tokio executor.
 *   3. Construct a `dyn ContextManager` from the remote gRPC service.
 *   4. Write the resulting Arc handle into `*out`.
 */
AmplifierResult amplifier_load_grpc_context(AmplifierHandle runtime,
                                            const char *endpoint,
                                            AmplifierHandle *out);

/**
 * Load a gRPC-backed approval module from the given endpoint.
 *
 * Validates that `runtime`, `endpoint`, and `out` are all non-null, and that
 * `endpoint` is valid UTF-8.
 *
 * # Returns
 *
 * - `ERR_NULL_HANDLE` if any argument is null.
 * - `ERR_INTERNAL` after argument validation (TODO: gRPC transport integration).
 *
 * # TODO
 *
 * Once a gRPC approval transport adapter is available, replace the
 * `ERR_INTERNAL` return with:
 *   1. Parse `endpoint` into a connection URI.
 *   2. Establish a gRPC channel using the runtime's Tokio executor.
 *   3. Construct a `dyn ApprovalHandler` from the remote gRPC service.
 *   4. Write the resulting Arc handle into `*out`.
 */
AmplifierResult amplifier_load_grpc_approval(AmplifierHandle runtime,
                                             const char *endpoint,
                                             AmplifierHandle *out);

/**
 * Start the kernel service for the given session, listening on `port`.
 *
 * Validates that `session` and `out_token` are non-null.
 *
 * On success, writes a service token handle into `*out_token` that can be
 * used to identify or stop the service later.
 *
 * # Returns
 *
 * - `ERR_NULL_HANDLE` if `session` or `out_token` is null.
 * - `ERR_INTERNAL` after argument validation (TODO: kernel service integration).
 *
 * # TODO
 *
 * Once the kernel service API is available, replace the `ERR_INTERNAL`
 * return with:
 *   1. Obtain the `FfiSession` Arc via `handle_to_arc_ref::<FfiSession>(session)`.
 *   2. Lock `session_arc.session`.
 *   3. Start a kernel service listener on `port`.
 *   4. Write the service token Arc handle into `*out_token`.
 */
AmplifierResult amplifier_kernel_service_start(AmplifierHandle session,
                                               uint16_t port,
                                               AmplifierHandle *out_token);

/**
 * Stop the kernel service associated with `session`.
 *
 * Validates that `session` is non-null.
 *
 * # Returns
 *
 * - `ERR_NULL_HANDLE` if `session` is null.
 * - `ERR_INTERNAL` after argument validation (TODO: kernel service integration).
 *
 * # TODO
 *
 * Once the kernel service API is available, replace the `ERR_INTERNAL`
 * return with:
 *   1. Obtain the `FfiSession` Arc via `handle_to_arc_ref::<FfiSession>(session)`.
 *   2. Lock `session_arc.session`.
 *   3. Stop the running kernel service listener.
 */
AmplifierResult amplifier_kernel_service_stop(AmplifierHandle session);

/**
 * Register a named capability on the session with a JSON value.
 *
 * Validates that `session`, `name`, and `value_json` are all non-null, and
 * that `name` and `value_json` are valid UTF-8.
 *
 * # Returns
 *
 * - `ERR_NULL_HANDLE` if any argument is null.
 * - `ERR_INTERNAL` after argument validation (TODO: kernel capability integration).
 *
 * # TODO
 *
 * Once the kernel capability API is available, replace the `ERR_INTERNAL`
 * return with:
 *   1. Obtain the `FfiSession` Arc via `handle_to_arc_ref::<FfiSession>(session)`.
 *   2. Lock `session_arc.session`.
 *   3. Parse `value_json` as a JSON value.
 *   4. Call `session.register_capability(_name_str, _value_json)`.
 */
AmplifierResult amplifier_register_capability(AmplifierHandle session,
                                              const char *name,
                                              const char *value_json);

/**
 * Retrieve a named capability from the session as a JSON string.
 *
 * Validates that `session`, `name`, and `out_json` are all non-null, and
 * that `name` is valid UTF-8.
 *
 * On success, writes a heap-allocated C string pointer containing the
 * capability's JSON value into `*out_json`. The caller must free the
 * returned string with `amplifier_string_free`.
 *
 * # Returns
 *
 * - `ERR_NULL_HANDLE` if any argument is null.
 * - `ERR_INTERNAL` after argument validation (TODO: kernel capability integration).
 *
 * # TODO
 *
 * Once the kernel capability API is available, replace the `ERR_INTERNAL`
 * return with:
 *   1. Obtain the `FfiSession` Arc via `handle_to_arc_ref::<FfiSession>(session)`.
 *   2. Lock `session_arc.session`.
 *   3. Call `session.get_capability(_name_str)`.
 *   4. Serialize the result to JSON and write a C string pointer into `*out_json`.
 */
AmplifierResult amplifier_get_capability(AmplifierHandle session,
                                         const char *name,
                                         char **out_json);

#endif  /* AMPLIFIER_FFI_H */
