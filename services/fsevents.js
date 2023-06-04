class Service extends ServiceBase {
	constructor(...args) {
		super(...args);
		this.watcher = new FileWatcher();
		this.watcher.add_listener(this.file_event.bind(this));
		this.watcher.init();
		this.watcher.start().catch((err) => {
			throw err;
		});
	}

	async file_event(event) {
		let op;
		let file;
		let dest;

		if (event.remove) {
			file = event.remove;
			op = api.FileEvent.Op.Remove;
		} else if (event.create) {
			file = event.create;
			op = api.FileEvent.Op.Create;
		} else if (event.modify) {
			file = event.modify;
			op = api.FileEvent.Op.Modify;
		} else if (event.rename) {
			file = event.rename[0];
			dest = event.rename[1];

			op = api.FileEvent.Op.Move;
		} else {
			console.error("Unknown fsevent:", event)
			return
		}

		this.send(
			api.Command.create({
				fileEvent: { 
					op,
					file: { path: file },
					dest: { path: dest }
				},
			}),
			0,
		);
	}

	async recv(cmd, session) {
		if (cmd.subscribeFile) {
			await this.watcher.watch(cmd.subscribeFile.files.map(e => e.path));
		}
	}
}

const service = new Service(
	serviceInfo.id,
	serviceInfo.service,
	serviceInfo.name,
);
await service.start();
