import { BN, creditcoinApi } from 'creditcoin-js';
import { ApiPromise, Keyring } from '@polkadot/api';
import * as fs from 'fs';
import * as child_process from 'child_process';
import { promisify } from 'util';

// From https://github.com/chevdor/subwasm/blob/c2e5b62384537875bfd0497c2b2d706265699798/lib/src/runtime_info.rs#L8-L20
/* eslint-disable @typescript-eslint/naming-convention */
type WasmRuntimeInfo = {
    size: number;
    compression: {
        size_compressed: number;
        size_decompressed: number;
        compressed: boolean;
    };
    reserved_meta: number[];
    reserved_meta_valid: boolean;
    metadata_version: number;
    core_version: string;
    proposal_hash: string;
    parachain_authorize_upgrade_hash: string;
    ipfs_hash: string;
    blake2_256: string;
};
/* eslint-enable */

// these normally use callbacks, but promises are more convenient
const readFile = promisify(fs.readFile);
const exec = promisify(child_process.exec);

type OldWeight = BN;
type NewWeight = { refTime: BN; proofSize: BN };
type Weight = OldWeight | NewWeight;

function createOverrideWeight(api: ApiPromise): Weight {
    const sudoCallTypeId = api.runtimeMetadata.registry.metadata.pallets
        .find((v) => v.name.toString() === 'Sudo')
        .calls.unwrap().type;
    const sudoCallTypeDef = api.runtimeMetadata.registry.lookup.getTypeDef(sudoCallTypeId);
    if (Array.isArray(sudoCallTypeDef.sub)) {
        const sudoUncheckedWeightType = sudoCallTypeDef.sub.find((def) => def.name === 'sudo_unchecked_weight');
        if (Array.isArray(sudoUncheckedWeightType.sub)) {
            // the weight is the second argument to sudo_unchecked_weight
            const weightType = sudoUncheckedWeightType.sub[1];
            if (weightType.type === 'u64') {
                // old weight (simple u64)
                return new BN(1);
            } else {
                // new weight
                return {
                    refTime: new BN(1),
                    proofSize: new BN(0),
                };
            }
        }
    }

    throw new Error("Couldn't find expected Weight type from sudoUncheckedWeight metadata");
}

/**
 * Performs an upgrade to the runtime at the provided path.
 * @param wsUrl The URL of the node to send the upgrade transaction to. Should be a websocket URL, like `ws://127.0.0.1:9944`
 * @param wasmBlobPath The path to the wasm blob to upgrade to.
 * @param sudoKeyUri The the secret key (SURI, either a mnemonic or raw secret) of the account to use to send the upgrade transaction.
 * Must be the sudo account.
 * @param hasSubwasm Whether the subwasm CLI tool is installed. If true subwasm is used to get info about the runtime and checks are performed.
 */
async function doRuntimeUpgrade(
    wsUrl: string,
    wasmBlobPath: string,
    sudoKeyUri: string,
    hasSubwasm = false,
    scheduleDelay = 50,
): Promise<void> {
    // init the api client
    const { api } = await creditcoinApi(wsUrl);
    try {
        // make the keyring for the sudo account
        const keyring = new Keyring({ type: 'sr25519' }).createFromUri(sudoKeyUri);

        const { specVersion } = api.runtimeVersion;

        let needsUpgrade = true;

        if (hasSubwasm) {
            // subwasm needs to be installed with `cargo install --locked --git https://github.com/chevdor/subwasm --tag v0.17.1`
            const output = await exec(`subwasm info -j ${wasmBlobPath}`);
            if (output.stderr.length > 0) {
                throw new Error(`subwasm info failed: ${output.stderr}`);
            }
            const info = JSON.parse(output.stdout) as WasmRuntimeInfo;
            // should probably do some checks here to see that the runtime is right
            // e.g. the core version is reasonable, it's compressed, etc.
            const [version] = info.core_version.split(' ');
            const [, versionNumString] = version.match(/(?:\w+\-)+(\d+)/);
            const versionNum = Number(versionNumString);

            if (versionNum <= specVersion.toNumber()) {
                needsUpgrade = false;
            }
        }

        if (!needsUpgrade) {
            console.log('Skipping upgrade because version has not increased');
            return;
        }

        // read the wasm blob from the give path
        const wasmBlob = await readFile(wasmBlobPath);

        const u8aToHex = (bytes: Uint8Array | Buffer): string => {
            const byteArray = Uint8Array.from(bytes);
            return byteArray.reduce((str, byte) => str + byte.toString(16).padStart(2, '0'), '0x');
        };

        const hexBlob = u8aToHex(wasmBlob);
        let callback = api.tx.system.setCode(hexBlob);
        if (scheduleDelay > 0) {
            callback = api.tx.scheduler.scheduleAfter(scheduleDelay, null, 0, callback);
        }
        const overrideWeight = createOverrideWeight(api);
        // schedule the upgrade
        await new Promise<void>((resolve, reject) => {
            const unsubscribe = api.tx.sudo
                .sudoUncheckedWeight(callback, overrideWeight)
                .signAndSend(keyring, { nonce: -1 }, (result) => {
                    const finish = (fn: () => void) => {
                        unsubscribe
                            .then((unsub) => {
                                unsub();
                                fn();
                            })
                            .catch(reject);
                    };
                    if (result.isInBlock && !result.isError) {
                        console.log('Runtime upgrade successfully scheduled');
                        finish(resolve);
                    } else if (result.isError) {
                        const error = new Error(`Failed to schedule runtime upgrade: ${result.toString()}`);
                        finish(() => reject(error));
                    }
                });
        });

        // WARNING: only used during fork-and-migrate testing
        if (scheduleDelay === 0) {
            callback = api.tx.posSwitch.switchToPos();

            await new Promise<void>((resolve, reject) => {
                const unsubscribe = api.tx.sudo
                    .sudoUncheckedWeight(callback, overrideWeight)
                    .signAndSend(keyring, { nonce: -1 }, (result) => {
                        const finish = (fn: () => void) => {
                            unsubscribe
                                .then((unsub) => {
                                    unsub();
                                    fn();
                                })
                                .catch(reject);
                        };
                        if (result.isInBlock && !result.isError) {
                            console.log('switchToPos called');
                            finish(resolve);
                        } else if (result.isError) {
                            const error = new Error(`Failed calling switchToPos: ${result.toString()}`);
                            finish(() => reject(error));
                        }
                    });
            });
        }
    } finally {
        await api.disconnect();
    }
}

if (process.argv.length < 5) {
    console.error('runtimeUpgrade.ts <wsUrl> <wasmBlobPath> <sudoKeyUri>');
    process.exit(1);
}

const inputWsUrl = process.argv[2];
const inputWasmBlobPath = process.argv[3];
const inputSudoKeyUri = process.argv[4];
const explicitDelay = Number(process.argv[5] || 50);

doRuntimeUpgrade(inputWsUrl, inputWasmBlobPath, inputSudoKeyUri, true, explicitDelay).catch((reason) => {
    console.error(reason);
    process.exit(1);
});
