import { DurableObject, WorkerEntrypoint } from "cloudflare:workers";
$WORKFLOW_IMPORT
import * as exports from "./index.js";

class Entrypoint extends WorkerEntrypoint {}

$HANDLERS

export default Entrypoint;
