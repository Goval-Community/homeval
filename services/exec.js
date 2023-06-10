class Service extends ServiceBase {
    constructor(...args) {
        super(...args)
        this.running = false
        this.proc = null
        this.queue = []

        this.dead_procs = []
        this.current_ref = null
    }
    
	async recv(cmd, session) {
		if (cmd.input) {
			await this.proc.write(cmd.input)
		} else if (cmd.exec) {
            let invalid = await this.validate_exec(cmd.exec)
            if (invalid) {
                return invalid
            }

            if (this.running) {
                const is_blocking = cmd.exec.blocking || (cmd.exec.lifecycle ===  api.Exec.Lifecycle.BLOCKING)

                if (!is_blocking) {
                    return api.Command.create({error: "Already running"})
                }

                this.queue.push(cmd)
                return
            }

            this.current_ref = cmd.ref
            await this.start_proc(cmd.exec)
        } else {
            console.debug("Unknown exec msg", cmd)
        }
	}

    async process_dead(proc_id, exit_code) {
        if (this.dead_procs.includes(proc_id) && proc_id !== -1) {return}
        this.dead_procs.push(proc_id)
        this.running = false
        
        if (proc_id !== -1) {
            try {
                await this.proc.destroy()
            } catch (_) {}
        }

        await this.send(api.Command.create({state: api.State.Stopped}), 0)

        let final_exit = api.Command.create({ok: {}})
        if (exit_code !== 0) {
            final_exit = api.Command.create({error: `exit status ${exit_code}`})
        }

        final_exit.ref = this.current_ref
        await this.send(final_exit, 0)

        
        this.current_ref = null
        if (this.queue.length === 0) {
            return
        }
        
        let cmd = this.queue.shift();
        this.current_ref = cmd.ref
        await this.start_proc(cmd.exec)
    }

    async validate_exec(cmd) {
        if (cmd.args.length === 0) {
            return api.Command.create({error: "Missing command"})
        }
        return false
    }
    
    async resource_usage(cmd) {
        const is_cpu_req = cmd === "date '+%s%N' && cat /sys/fs/cgroup/cpu/cpuacct.usage /sys/fs/cgroup/cpu/cpu.cfs_quota_us /sys/fs/cgroup/cpu/cpu.cfs_period_us /sys/fs/cgroup/memory/memory.usage_in_bytes /sys/fs/cgroup/memory/memory.soft_limit_in_bytes /sys/fs/cgroup/memory/memory.limit_in_bytes &&grep '^\\(total_rss\\|total_cache\\) ' /sys/fs/cgroup/memory/memory.stat";
        const is_storage_req = cmd === "cat /repl/stats/subvolume_usage_bytes /repl/stats/subvolume_total_bytes";
        // if (is_cpu_req) {
        //     return "date '+%s%N' && echo 100000 && echo 200000 && cat /sys/fs/cgroup/cpu/cpu.cfs_period_us /sys/fs/cgroup/memory/memory.usage_in_bytes /sys/fs/cgroup/memory/memory.soft_limit_in_bytes /sys/fs/cgroup/memory/memory.limit_in_bytes &&grep '^\\(total_rss\\|total_cache\\) ' /sys/fs/cgroup/memory/memory.stat";
        // } else 
        if (is_cpu_req || is_storage_req) {
            await this.send(api.Command.create({state: api.State.Running}), 0)
            let output = "";

            if (is_storage_req) {
                const disk = await Deno.core.ops.op_disk_info();
                output = `${disk.free}\n${disk.total}\n`
            }
            if (is_cpu_req) {
                const cpuTime = await Deno.core.ops.op_cpu_info();
                const memory = await Deno.core.ops.op_memory_info();
                const memoryUsage = memory.total - memory.free;
                const totalMemory = memory.total
                output = `${new Number(Date.now()) * 1000000}\n${cpuTime}\n200000\n100000\n${memoryUsage}\n${totalMemory}\n${totalMemory}\ntotal_cache 0\ntotal_rss ${memoryUsage}`
            }

            await this.send(api.Command.create({output}), 0)
            
            await this.process_dead(-1, 0)

            return false
        } else {
            return cmd
        }
    }

    async start_proc(msg) {
        this.running = true
        // TODO: splitLogs, splitStderr
        let cmd = msg.args[0];
        let env = msg.env ? msg.env : {};
        env.REPLIT_GIT_ASKPASS_GODS_PLS_SEND_TO_RIGHT_SESSION_SHELL_TOKEN = this.id.toString()

        if (msg.args.length === 3 && msg.args[1] === "-c") {
            const search = "rg --json --context 0 --fixed-strings" 
            if (cmd === "bash") {
                let arg = msg.args[2]

                let new_cmd = await this.resource_usage(arg)
                if (new_cmd) {
                    msg.args[2] = new_cmd
                } else {
                    return
                }
            } else if (cmd === "sh") {
                if (msg.args[2].slice(0, search.length) === search) {
                    await this.send(api.Command.create({state: api.State.Running}), 0)
                    msg.args[0] = "bash"
                    const exit = await Deno.core.ops.op_run_cmd(msg.args, this.id, this.clients, env)

                    await this.process_dead(-1, exit)
                    return 
                }
            }
            
        }

        const args = msg.args.slice(1)

        await this.send(api.Command.create({state: api.State.Running}), 0)
        this.proc = new Process(this.id, cmd, args, env)
        
        await this.proc.init(this.clients)

    }

    async attach(session) {
        if (this.proc) await this.proc.add_session(session)
    }

    async detach(session) {
        if (this.proc) await this.proc.remove_session(session)
    }
}

const service = new Service(
	serviceInfo.id,
	serviceInfo.service,
	serviceInfo.name,
);
await service.start()
