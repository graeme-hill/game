#!/usr/bin/env python3
"""Exercise animation authoring and third-person controls with real window input."""
import hashlib
import json
import os
from pathlib import Path
import select
import shutil
import subprocess
import time
from PIL import Image
from creator_smoke import Session, float32_model
from visual_support import ROOT, stop, wait_for


def main():
    output = ROOT / 'artifacts' / f'motion-{time.strftime("%Y%m%d-%H%M%S")}-{os.getpid()}'
    output.mkdir(parents=True)
    data = output / 'data'
    shutil.copytree(ROOT / 'test_workspace', data)
    report = dict(status='failed', renderer='Mesa software Vulkan', actions=[], captures=[])
    env = os.environ.copy()
    env['XDG_CACHE_HOME'] = str(ROOT / 'artifacts/cache')
    drivers = [p for d in env.get('XDG_DATA_DIRS', '/usr/share').split(':') for p in Path(d).glob('vulkan/icd.d/lvp*.json')]
    env.update(VK_DRIVER_FILES=str(drivers[-1]), VK_ICD_FILENAMES=str(drivers[-1]),
               WINIT_UNIX_BACKEND='x11', WINIT_X11_SCALE_FACTOR='1', WGPU_BACKEND='vulkan',
               RUST_LOG='info,wgpu_core=warn,wgpu_hal=warn', RUST_BACKTRACE='1')
    env.pop('WAYLAND_DISPLAY', None)
    xvfb = session = game = None
    try:
        rd, wr = os.pipe()
        with (output / 'xvfb.log').open('w') as log:
            xvfb = subprocess.Popen(['Xvfb', '-displayfd', str(wr), '-screen', '0', '1600x900x24', '-nolisten', 'tcp', '-ac'],
                                    pass_fds=(wr,), env=env, stdout=log, stderr=subprocess.STDOUT)
        os.close(wr)
        assert select.select([rd], [], [], 15)[0], 'Xvfb startup'
        env['DISPLAY'] = ':' + os.read(rd, 64).decode().strip()
        os.close(rd)
        session = Session(env, output, data, report, workspace='characters', prefix='editor')
        s = session
        s.action('Animation(Tools(true))')
        assert [c['name'] for c in s.state()['library']['characters'][0]['animations']] == ['standing', 'idle', 'walking', 'running']
        # Cycle through actual visible controls to reach walking.
        s.action('Animation(Select(1))')
        s.action('Animation(Select(2))')
        s.action('Animation(Step(0.1))')
        s.action('Animation(Step(0.1))')
        s.capture('walking-editor')
        for _ in range(3):
            s.action(next(c['action'] for c in s.state()['controls'] if c['label'] == 'Blend with >'))
        for _ in range(5):
            s.action('Animation(Blend(0.1))')
        assert abs(s.state()['animation']['blend'] - .5) < .001
        s.capture('walk-run-blend')
        s.action('Animation(New)')
        s.action('Rename(Animation)')
        s.xdo('key', 'ctrl+a')
        s.wait(lambda v: v.get('naming') == '', 'clear clip name')
        s.xdo('type', '--delay', 65, 'Wave')
        s.xdo('key', 'Return')
        s.wait(lambda v: v.get('naming') is None, 'commit name')
        s.action('Animation(Key)')
        s.action('Animation(Step(0.1))')
        adjust = next(c['action'] for c in s.state()['controls'] if c['action'].startswith('Animation(Adjust(0, 0.08'))
        s.action(adjust)
        expected = s.state()['library']
        clip = expected['characters'][0]['animations'][-1]
        assert clip['name'] == 'Wave' and len(clip['tracks'][0]['keys']) == 2
        s.action('Animation(Duplicate)')
        s.action('Undo')
        assert s.state()['library'] == expected
        s.action('Animation(Duration(0.1))')
        s.action('Animation(Loop)')
        s.action('Animation(Play)')
        s.wait(lambda v: v['animation']['time'] > .3, 'playback cursor')
        s.action('Animation(Play)')
        s.action('Save')
        expected = s.state()['library']
        s.capture('animation-authored')
        # Every animation control stays available in the live resized window.
        for width, height in [(800,600), (640,480), (1280,720)]:
            s.xdo('windowsize', '--sync', s.window, width, height)
            state = s.wait(lambda v: v.get('window') == [width,height], 'animation resize')
            for c in state['controls']:
                x,y = c['center']
                assert 0 <= x < width and 0 <= y < height, (width,height,c)
            s.capture_number += 1
            s.xdo('key', 'F12')
            path = output / f'screenshot-{s.capture_number:03}.png'
            def complete():
                try:
                    with Image.open(path) as im:
                        im.load()
                        return im.size == (width,height)
                except (OSError,ValueError): return False
            wait_for(complete, 'resized animation capture', s.process)
            path.rename(output / f'animation-{width}x{height}.png')
        stop(s.process)
        session = Session(env, output, data, report, workspace='characters', prefix='reopened')
        assert float32_model(session.state()['library']) == float32_model(expected), 'Animation save/restart'
        session.action('Animation(Tools(true))')
        session.capture('animation-reopened')
        stop(session.process)
        original = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in data.iterdir() if p.is_file()}
        state_path = output / 'play-state.json'
        with (output / 'play.log').open('w') as log:
            game = subprocess.Popen([str(ROOT/'target/debug/game'), '--mode', 'game', '--workspace', str(data),
                                     '--state-file', str(state_path), '--output-dir', str(output/'play'),
                                     '--capture', str(output/'play-ready.png'), '--capture-frame', '120'],
                                    cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT)
        def state():
            try: return json.loads(state_path.read_text())
            except (ValueError,OSError): return {}
        def wait(predicate, label):
            wait_for(lambda: predicate(state()), label, game, timeout=120)
            return state()
        wait(lambda v: v.get('spawned_parts') == 1, 'play startup')
        wait_for(lambda: 'capture_saved' in (output/'play.log').read_text(), 'play warmup', game)
        def xdo(*args):
            return subprocess.check_output(['xdotool', *map(str,args)], env=env, text=True, timeout=10).strip()
        window = xdo('search','--sync','--onlyvisible','--pid',game.pid).splitlines()[0]
        xdo('windowfocus','--sync',window)
        xdo('mousemove','--window',window,640,360)
        xdo('click',1)
        wait(lambda v:v.get('captured'), 'mouse capture')
        wait(lambda v:v['weights'][1] > .9, 'idle animation')
        captures = 0
        def capture(name):
            nonlocal captures
            captures += 1
            xdo('key','F12')
            path = output/'play'/f'screenshot-{captures:03}.png'
            def complete():
                try:
                    with Image.open(path) as im:
                        im.load()
                        assert im.size == (1280,720)
                    return True
                except (ValueError,OSError): return False
            wait_for(complete,name,game)
            shutil.copyfile(path,output/f'{name}.png')
            (output/f'{name}-state.json').write_text(json.dumps(state(),indent=2))
        capture('play-idle')
        start = state()['position']
        xdo('keydown','w')
        wait(lambda v: v['speed'] > 2.5 and v['weights'][2] > .85, 'walking')
        capture('play-walking')
        xdo('keydown','Shift_L')
        wait(lambda v:v['speed'] > 6.4 and v['weights'][3] > .85,'running')
        capture('play-running')
        xdo('keyup','w','Shift_L')
        wait(lambda v:v['speed'] < .01,'braking')
        assert state()['position'][2] < start[2] - 1
        yaw = state()['camera_yaw']
        xdo('mousemove_relative','--',120,40)
        wait(lambda v:abs(v['camera_yaw'] - yaw) > .1,'mouse orbit')
        distance = state()['camera_distance']
        xdo('click',4)
        wait(lambda v:v['camera_distance'] < distance - .2,'mouse zoom')
        xdo('key','space')
        wait(lambda v:not v['grounded'],'jump')
        capture('play-jump')
        wait(lambda v:v['grounded'],'landing')
        xdo('key','r')
        wait(lambda v:abs(v['camera_yaw']-v['facing']) < .01,'recenter')
        xdo('key','Escape')
        wait(lambda v:not v['captured'],'release cursor')
        before = state()['position']
        xdo('keydown','w'); time.sleep(.4); xdo('keyup','w')
        assert state()['position'] == before,'released controls moved player'
        xdo('click',1)
        wait(lambda v:v['captured'],'recapture')
        assert original == {p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in data.iterdir() if p.is_file()}
        for name in ['editor.log','reopened.log','play.log']:
            text = (output/name).read_text()
            assert 'ERROR' not in text and 'panicked at' not in text, name
        report.update(status='passed', gameplay=['idle','walk','run','jump','land','orbit','zoom','recenter','release','recapture'],
                      gamepad='Synthetic Bevy input covered by Rust tests; no physical controller tested')
        print('PASS: animation authoring/save/restart and third-person keyboard/mouse controls.',flush=True)
    except Exception as error:
        report['error'] = repr(error)
        raise
    finally:
        if session: stop(session.process)
        stop(game); stop(xvfb)
        (output/'report.json').write_text(json.dumps(report,indent=2)+'\n')
        print(f'Report: {output / "report.json"}',flush=True)


if __name__ == '__main__':
    main()
