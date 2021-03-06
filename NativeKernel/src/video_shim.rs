//!
//!
//!
use ::kernel::metadevs::video;
use ::gui::input::keyboard as gui_keyboard;
use ::gui::input::keyboard::KeyCode;
use ::std::sync::Arc;
use ::std::sync::Mutex;

/// Structure for the window-based console
pub struct Console
{
	_worker: ::std::thread::JoinHandle<()>,
	state: Arc<Mutex<State>>,
}
struct State
{
	/// Current size of the window
    size: video::Dims,
	/// Backing buffer
	backbuffer: Vec<u32>,
	/// Indicates that the buffer has changed since it was last sent to the window
	dirty: bool,
}
impl Console
{
    pub fn new() -> Arc<Self>
	{
		let state = Arc::new(Mutex::new(State {
			size: video::Dims::new(640, 480),
			backbuffer: ::std::vec![ 0u32; 640 * 480 ],
			dirty: true,
			}));

		let (input_tx, input_rx) = ::std::sync::mpsc::channel::<(bool,KeyCode,)>();
		// A kernel thread to handle sending keystrokes to the GUI
		let input_worker = ::kernel::threads::WorkerThread::new("GUI Input", move || {
			let gui_keyboard = gui_keyboard::Instance::new();
			loop
			{
				let (is_release, key) = ::kernel::arch::imp::threads::test_pause_thread(|| input_rx.recv().expect("Input sender dropped") );

				if is_release {
					gui_keyboard.release_key(key);
				}
				else {
					gui_keyboard.press_key(key);
				}
			}
			});

		Arc::new(Console {
			state: state.clone(),
			_worker: ::std::thread::spawn(move || {
				let _ = input_worker;
				let size = state.lock().unwrap().size;
				let mut window = ::minifb::Window::new("RustOS Native", size.width() as usize, size.height() as usize, Default::default()).expect("Failed to spawn window");

				window.limit_update_rate(Some(::std::time::Duration::from_millis(16)));
				let mut prev_keys = ::std::vec![];
				loop {
					window.update();

					{
						let mut lh = state.lock().unwrap();
						if ::std::mem::replace(&mut lh.dirty, false) {
							window.update_with_buffer(&lh.backbuffer, lh.size.width() as usize, lh.size.height() as usize);
						}
					}

					// TODO: Mouse handling

					if let Some(mut keys) = window.get_keys()
					{
						if keys != prev_keys
						{
							log_debug!("keys = {:?} -> {:?}", prev_keys, keys);
						}

						fn translate_keycode(k: &::minifb::Key) -> KeyCode {
							use ::minifb::Key;
							macro_rules! keymap {
								( $($k:ident $(=> $k2:ident)? ,)* ) => {
									match k
									{
									$(Key::$k => keymap!(@sel $k $($k2)?) ,)*
									_ => KeyCode::None,
									}
								};
								(@sel $k:ident) => { KeyCode::$k };
								(@sel $k:ident $k2:ident) => { KeyCode::$k2 };
							}
							keymap!(
								A, B, C, D, E, F, G, H, I, J, K, L, M,
								N, O, P, Q, R, S, T, U, V, W, X, Y, Z,
								Enter => Return,
								LeftCtrl,
								)
						}

						// Get the difference between the two
						keys.sort();
						let mut it_p = prev_keys.iter();
						let mut it_n = keys.iter();

						let mut cur_p = it_p.next();
						let mut cur_n = it_n.next();
						loop
						{
							let (is_release, minifb_key) = match (cur_p, cur_n)
								{

								// `p` is released
								(Some(p), Some(n)) if p < n => {
									cur_p = it_p.next();
									(true, p)
									},
								(Some(p), None) => {
									cur_p = it_p.next();
									(true, p)
									},

								// `n` has been pressed
								(Some(p), Some(n)) if p > n => {
									cur_n = it_n.next();
									(false, n)
									},
								(None, Some(n)) => {
									cur_n = it_n.next();
									(false, n)
									},

								(Some(p), Some(n)) => {
									assert!( p == n );
									cur_p = it_p.next();
									cur_n = it_n.next();
									continue ;
									},

								(None, None) => break,
								};
							let key = translate_keycode(minifb_key);
							if key == KeyCode::None {
								log_notice!("No translation for minifb::Key::{:?} -> gui::KeyCode", minifb_key)
							}
							else {
								input_tx.send( (is_release, key) ).expect("Input worker quit");
							}
						}
						
						prev_keys = keys;
					}
				}
				}),
			})
	}

    pub fn get_display(&self) -> Display
    {
        Display {
			state: self.state.clone(),
        }
    }
}

/// Display surface backed by a window
pub struct Display
{
	state: Arc<Mutex<State>>,
}
impl video::Framebuffer for Display
{
	fn as_any(&self) -> &dyn std::any::Any {
        self
    }
	fn activate(&mut self) {
        // Anything?
    }
	
	fn get_size(&self) -> video::Dims {
		self.state.lock().unwrap().size
    }
	fn set_size(&mut self, newsize: video::Dims) -> bool  {
        todo!("set_size: {:?}", newsize)
    }
	
	fn blit_inner(&mut self, dst: video::Rect, src: video::Rect) {
        todo!("")
    }
	fn blit_ext(&mut self, dst: video::Rect, src: video::Rect, srf: &dyn video::Framebuffer) -> bool {
        todo!("")
    }
	fn blit_buf(&mut self, dst: video::Rect, buf: video::StrideBuf<'_,u32>) {
		let mut lh = self.state.lock().unwrap();
		let backbuffer_w = lh.size.width() as usize;
		for (row,src) in ::kernel::lib::ExactZip::new( dst.top() .. dst.bottom(), buf.chunks(dst.w() as usize) )
		{
			//let seg = self.buffer.scanline_slice(row as usize, dst.left() as usize, dst.right() as usize);
			let seg = &mut lh.backbuffer[row as usize * backbuffer_w ..][dst.left() as usize .. dst.right() as usize];
			seg.copy_from_slice( src );
		}
		lh.dirty = true;
    }
	fn fill(&mut self, dst: video::Rect, colour: u32) {
        todo!("fill({:?}, {:#x})", dst, colour)
    }
	
	fn move_cursor(&mut self, p: Option<video::Pos>) {
        todo!("")
    }
}
