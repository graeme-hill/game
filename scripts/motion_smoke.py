#!/usr/bin/env python3
"""Exercise animation authoring and third-person controls with real window input."""
import hashlib
import json
import math
import os
import shutil
import subprocess
import time
from PIL import Image
from creator_smoke import Session, float32_model
from visual_support import ROOT, game_binary, software_vulkan, start_xvfb, stop, wait_for


def main():
    output = ROOT / 'artifacts' / f'motion-{time.strftime("%Y%m%d-%H%M%S")}-{os.getpid()}'
    output.mkdir(parents=True)
    data = output / 'data'
    shutil.copytree(ROOT / 'test_workspace', data)
    report = dict(status='failed', renderer='Mesa software Vulkan', actions=[], captures=[])
    env = software_vulkan(os.environ.copy())
    xvfb = session = game = None
    try:
        xvfb = start_xvfb(env, (1600, 900), output / 'xvfb.log')
        session = Session(env, output, data, report, workspace='characters', prefix='editor')
        s = session
        character = s.library()['characters'][0]
        assert [c['name'] for c in character['animations']] == ['standing', 'idle', 'walking', 'running']
        # Clips open from the Outline; the timeline docks under the viewport.
        s.action('Select(Clip(1))')
        s.action('Select(Clip(2))')
        assert s.state()['animation']['clip'] == 2
        for _ in range(3):
            s.xdo('key', 'Right')
        s.wait(lambda v: v['animation']['time'] > 0.09, 'keyboard playhead steps')
        s.capture('walking-editor')
        # A right-drag orbits the preview; the View menu snaps to a side view
        # because front/rear screenshots cannot reveal reversed knees.
        yaw = s.state()['preview_camera']['yaw']
        x, y, width, height = s.state()['viewport']
        s.xdo('mousemove', '--window', s.window, round(x + width / 2), round(y + height / 2))
        time.sleep(.1)
        s.xdo('mousedown', 3)
        s.xdo('mousemove_relative', '--', 80, 0)
        time.sleep(.2)
        s.xdo('mouseup', 3)
        s.wait(lambda v: abs(v['preview_camera']['yaw'] - yaw) > .2, 'right-drag orbit')
        s.menu('OpenMenu(View)', 'View(Side)')
        assert abs(s.state()['preview_camera']['yaw'] - math.pi / 2) < .025
        for _ in range(2):
            s.xdo('click', 4)
            time.sleep(.2)

        def timeline_time(seconds):
            control = s.control(lambda kind: kind == 'Timeline', by='kind')
            duration = s.library()['characters'][0]['animations'][s.state()['animation']['clip']]['duration']
            left = control['center'][0] - control['size'][0] / 2 + 1
            s.press({'center': [left + (control['size'][0] - 2) * seconds / duration, control['center'][1]],
                     'size': [1, 1]}, f'timeline {seconds:.1f}s')
            s.wait(lambda v: abs(v['animation']['time'] - seconds) < duration / control['size'][0] * 2,
                   f'playhead at {seconds}')
        for index, name in [(2, 'walk'), (3, 'run')]:
            s.action(f'Select(Clip({index}))')
            for pose in range(5):
                timeline_time(pose * 0.1)
                s.capture(f'side-{name}-{pose}')
        s.action('Select(Clip(2))')
        timeline_time(0.2)
        s.action('Frame')
        s.menu('OpenMenu(BlendTarget)', 'Anim(BlendWith(Some(3)))')
        assert abs(s.state()['animation']['blend'] - .5) < .001
        s.capture('walk-run-blend')
        s.action('Anim(New)')
        clip = s.state()['animation']['clip']
        s.edit_text('ClipName', 'Wave')
        bone = s.library()['bodies'][0]['bones'][0]['id']
        s.action(f'Select(Bone({bone}))')
        s.action('Anim(Key)')
        for _ in range(3):
            s.xdo('key', 'Right')
        s.wait(lambda v: v['animation']['time'] > 0.09, 'step to the second key')
        s.field('KeyRotation(0)', 30)
        expected = s.library()
        wave = expected['characters'][0]['animations'][-1]
        assert wave['name'] == 'Wave' and len(wave['tracks'][0]['keys']) == 2
        assert abs(wave['tracks'][0]['keys'][1]['rotation'][0] - math.radians(30)) < .001
        s.action(f'Select(Clip({clip}))')
        s.action('Anim(Duplicate)')
        s.action('Undo')
        assert s.library() == expected
        s.field('ClipDuration', 1.1)
        s.action('Anim(Loop)')
        s.action('Anim(Play)')
        s.wait(lambda v: v['animation']['time'] > .3, 'playback cursor')
        s.action('Anim(Play)')
        s.action('Save')
        expected = s.library()
        s.capture('animation-authored')
        # Every control stays on screen in the live resized window.
        for width, height in [(800, 600), (640, 480), (1280, 720)]:
            s.xdo('windowsize', '--sync', s.window, width, height)
            state = s.wait(lambda v: v.get('window') == [width, height], 'animation resize')
            time.sleep(0.5)
            state = s.settle()
            for c in state['controls']:
                x, y = c['center']
                assert 0 <= x < width and 0 <= y < height, (width, height, c)
            s.capture_number += 1
            s.xdo('key', 'F12')
            path = s.f12 / f'screenshot-{s.capture_number:03}.png'

            def complete():
                try:
                    with Image.open(path) as im:
                        im.load()
                        return im.size == (width, height)
                except (OSError, ValueError):
                    return False
            wait_for(complete, 'resized animation capture', s.process)
            path.rename(output / f'animation-{width}x{height}.png')
        stop(s.process)
        session = Session(env, output, data, report, workspace='characters', prefix='reopened')
        assert float32_model(session.state()['library']) == float32_model(expected), 'Animation save/restart'
        session.action('Select(Clip(4))')
        session.capture('animation-reopened')
        stop(session.process)
        def workspace_digest():
            return {str(p.relative_to(data)): hashlib.sha256(p.read_bytes()).hexdigest()
                    for p in data.rglob('*') if p.is_file()}
        original = workspace_digest()
        state_path = output / 'play-state.json'
        with (output / 'play.log').open('w') as log:
            game = subprocess.Popen([str(game_binary()), '--mode', 'game', '--workspace', str(data),
                                     '--state-file', str(state_path), '--output-dir', str(output/'play'),
                                     '--capture', str(output/'play-ready.png'), '--capture-frame', '120'],
                                    cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT)
        def state():
            try: return json.loads(state_path.read_text())
            except (ValueError,OSError): return {}
        def wait(predicate, label):
            wait_for(lambda: predicate(state()), label, game, timeout=120)
            return state()
        wait(lambda v: v.get('spawned_parts') == 3, 'play startup')
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
        def check_equipment():
            current = state()
            mounted = current['mounted_props']
            attachments = current['character']['attachments']
            assert len(mounted) == len(attachments) == 2
            assert {p['prop'] for p in mounted} == {a['prop'] for a in attachments}
            assert {p['attachment'] for p in mounted} == {a['id'] for a in attachments}
            assert all(math.isfinite(v) for p in mounted for field in ('translation', 'rotation', 'scale') for v in p[field])
            assert all(p['scale'][0] > 0 for p in mounted)
        def capture(name):
            nonlocal captures
            check_equipment()
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
        # Inspect both sides of the equipped sample using ordinary camera input.
        xdo('click', '--repeat', 3, '--delay', 150, 4)
        capture('equipment-back')
        yaw = state()['camera_yaw']
        for _ in range(40):
            if math.cos(state()['camera_yaw'] - yaw) < -.97:
                break
            previous = state()['camera_yaw']
            xdo('mousemove_relative', '--', 60, 0)
            wait(lambda v: abs(v['camera_yaw'] - previous) > .05, 'equipment orbit step')
        assert math.cos(state()['camera_yaw'] - yaw) < -.97, 'Camera did not reach the front'
        capture('equipment-front')
        # Camera-relative strafing presents an actual side-on travelling gait.
        xdo('keydown', 'd')
        wait(lambda v: v['weights'][2] > .95 and abs(math.cos(v['camera_yaw'] - v['facing'])) < .03, 'side walk')
        capture('play-side-walking')
        xdo('keydown', 'Shift_L')
        wait(lambda v: v['weights'][3] > .95, 'side run')
        capture('play-side-running')
        xdo('keyup', 'd', 'Shift_L')
        wait(lambda v: v['speed'] < .01, 'side gait braking')
        xdo('key', 'r')
        wait(lambda v: abs(v['camera_yaw'] - v['facing']) < .01, 'equipment recenter')
        xdo('click', 5)
        start = state()['position']
        xdo('keydown','w')
        wait(lambda v: v['speed'] > v['walk_speed'] * .96 and v['weights'][2] > .85, 'walking')
        capture('play-walking')
        xdo('keydown','Shift_L')
        wait(lambda v:v['speed'] > v['run_speed'] * .96 and v['weights'][3] > .85,'running')
        capture('play-running')
        xdo('keyup','w','Shift_L')
        wait(lambda v:v['speed'] < .01,'braking')
        assert sum((a-b)**2 for a,b in zip(state()['position'],start)) > 1, 'Walking/running did not move the character'
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
        assert original == workspace_digest(), 'Play changed workspace assets'
        for name in ['editor.log','reopened.log','play.log']:
            text = (output/name).read_text()
            errors = [line for line in text.splitlines() if (' ERROR ' in line and 'gilrs' not in line) or 'panicked at' in line]
            assert not errors, f'{name}: {errors[-1]}'
        report.update(status='passed', gameplay=['idle','walk','run','jump','land','orbit','zoom','recenter','release','recapture'],
                      gait=['side-view contact/down/passing/recovery previews', 'side-on keyboard walk/run'],
                      equipment=['strawberry hat', 'bee backpack', 'back/front orbit', 'mounted during idle/walk/run/jump'],
                      gamepad='Synthetic Bevy input covered by Rust tests; no physical controller tested')
        print('PASS: animation authoring/save/restart and third-person keyboard/mouse controls.',flush=True)
    except Exception as error:
        report['error'] = repr(error)
        if session is not None:
            (output / 'failure-state.json').write_text(json.dumps(session.state(), indent=2) + '\n')
        raise
    finally:
        if session: stop(session.process)
        stop(game); stop(xvfb)
        (output/'report.json').write_text(json.dumps(report,indent=2)+'\n')
        print(f'Report: {output / "report.json"}',flush=True)


if __name__ == '__main__':
    main()
